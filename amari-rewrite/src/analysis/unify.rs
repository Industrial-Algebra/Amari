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
//! ## Borrowed admission before any clone (F24, PR #384 round 5)
//!
//! [`unify_with`] is a public entry point over caller-supplied, unbounded terms
//! and a caller-supplied substitution, so nothing proportional to that input may
//! be cloned before it is admitted:
//!
//! 1. both borrowed input shapes are measured by the iterative, metered
//!    [`crate::inverse::measure_borrowed`] walk, which abandons the walk at the
//!    node or depth ceiling instead of allocating first;
//! 2. every pre-existing substitution range is measured the same way under the
//!    SAME [`RelationResources`] meter, so the aggregate admission is bounded
//!    rather than one fresh walk per binding;
//! 3. the pre-existing binding graph is admitted only if it is acyclic (see
//!    below);
//! 4. only then is an owned worklist built. Every child equation is charged one
//!    operation BEFORE its terms are cloned and enqueued, so a budget that
//!    cannot pay for its own pending work rejects the unification instead of
//!    growing a worklist without bound.
//!
//! `Term::clone` is recursive, so a depth-unbounded caller term used to overflow
//! the call stack here even with `max_operations = 1`. Admission is metered and
//! iterative, so the typed limit is observed before the clone.
//!
//! ## Reflective equations and duplicate pairs (F25, PR #384 round 5)
//!
//! Two equations are skipped before they cost anything beyond their own pop:
//!
//! - a **reflexive** equation `X = X` (the same variable on both sides) is
//!   decided before resolution, so a reflexive goal over a doubling substitution
//!   costs one operation instead of attempting `2^21 - 1` duplicate child pairs;
//! - an equation pair whose resolution is **stable** — the pair was already
//!   decided and the substitution has not changed since — is skipped before it
//!   is resolved again. The cache is invalidated on EVERY binding mutation, so
//!   no decision is ever reused across a mutation. The key is the equation pair
//!   the worklist already owns (moved into the cache, not cloned); a key derived
//!   from the resolved pair could only be built after paying the resolution walk
//!   the cache exists to avoid.
//!
//! ## Supported cyclic inputs (F23, PR #384 round 5)
//!
//! A successful [`unify`] / [`unify_with`] satisfies
//! `substitution.apply(left) == substitution.apply(right)` — the returned
//! substitution is an actual unifier, not merely a set of bindings that
//! resolves. A caller-supplied substitution may therefore only be **acyclic in
//! the binding graph**, with one exception:
//!
//! - an **identity** binding `X |-> X` is supported. [`Substitution::apply`]
//!   leaves `X` in place at its own fixed point, so `X` and `X` denote the same
//!   term and the identity is sound;
//! - any **non-identity cycle** reachable through the pre-existing bindings
//!   (for example `Y |-> g(Y)`, or `X |-> g(Y)` with `Y |-> X`) is NOT
//!   supported: [`Substitution::apply`] would report two distinct fixed points
//!   for the same instance, so no substitution could satisfy the postcondition
//!   above. Such input is rejected with a typed
//!   [`RewriteError::InvalidSubstitution`] BEFORE any binding is made.
//!
//! The rejection is a property of the caller's substitution, not of the
//! equation: an acyclic substitution always either unifies or fails with a typed
//! [`RewriteError::UnificationFailure`] (head/arity clash or occurs-check
//! violation), and the ordinary occurs-check semantics for NEW bindings are
//! unchanged.

use alloc::collections::BTreeSet;
use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use crate::error::{RewriteError, RewriteResult};
use crate::inverse::measure_borrowed;
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
/// On success the returned (extended) substitution is a genuine unifier:
/// `substitution.apply(left) == substitution.apply(right)`.
///
/// A pre-existing non-identity cycle in the binding graph cannot satisfy that
/// postcondition and is rejected with
/// [`RewriteError::InvalidSubstitution`]; an identity binding `X |-> X` is a
/// supported fixed point. See the module documentation.
///
/// On error the substitution may be partially extended; callers treat
/// failure as terminal for the current goal.
pub fn unify_with(
    substitution: &mut Substitution,
    left: &Term,
    right: &Term,
    resources: &mut RelationResources,
) -> RewriteResult<()> {
    // F24: admit every borrowed input BEFORE it is cloned. Nothing in this
    // function clones a caller term or range until this returns Ok.
    admit_borrowed_inputs(substitution, left, right, resources)?;
    let mut worklist: Vec<(Term, Term)> = Vec::new();
    push_pair(&mut worklist, left, right, resources)?;
    let mut seen = SeenEquations::new();
    while let Some(pair) = worklist.pop() {
        resources.record_operations(1)?;
        // F25: reflexive fast path, decided BEFORE resolution. A doubling
        // substitution makes the resolved pair `pair(X{k+1},X{k+1})` twice, so
        // resolving first used to enqueue both children and attempt
        // `2^21 - 1` duplicate pairs for `X0 = X0`.
        if same_variable(&pair.0, &pair.1) {
            continue;
        }
        // F25: an equation pair whose resolution is still stable — the
        // substitution has not changed since it was processed — is not
        // processed again. Every decision below is a pure function of the pair
        // and the substitution, so a remembered pair would only repeat the
        // resolution walk and re-push children that were already pushed.
        if seen.contains(&pair) {
            continue;
        }
        match resolve_pair(substitution, &pair.0, &pair.1, resources)? {
            Decision::Equal => {
                // Nothing to bind and nothing to decompose: remember the pair.
                seen.remember(pair);
            }
            Decision::Bind(variable, range) => {
                bind(substitution, &variable, range, resources)?;
                // The binding changed the substitution, so no remembered pair
                // may survive: every one of them is now stale.
                seen.invalidate();
            }
            Decision::Children(children) => {
                // The children are the children of the RESOLVED pair, so a
                // pre-bound variable whose range is a symbol is decomposed into
                // the range's arguments and never the variable itself.
                //
                // Charge-before-clone: each child equation is one pending pop,
                // so a budget that cannot pay for the children rejects here
                // instead of cloning and enqueueing them.
                for (left_arg, right_arg) in &children {
                    push_pair(&mut worklist, left_arg, right_arg, resources)?;
                }
                // Decomposed: the pair itself is decided, so remember it. A
                // child binding clears the cache, so this is never reused
                // across a mutation.
                seen.remember(pair);
            }
        }
    }
    Ok(())
}

/// The decision `resolve_pair` makes about one popped equation.
enum Decision {
    /// Both sides resolved to the same term: nothing to do.
    Equal,
    /// Bind `variable` to `range`.
    Bind(Variable, Term),
    /// Both sides resolved to symbols with the same head and arity: decompose
    /// into these child equations.
    Children(Vec<(Term, Term)>),
}

fn same_variable(left: &Term, right: &Term) -> bool {
    matches!((left, right), (Term::Var(left), Term::Var(right)) if left == right)
}

fn failure(reason: &str) -> RewriteError {
    RewriteError::UnificationFailure {
        reason: String::from(reason),
    }
}

fn cyclic(variable: &Variable) -> RewriteError {
    RewriteError::InvalidSubstitution {
        message: format!(
            "pre-existing binding cycle through `{}` is not an identity, so no unifier exists under fixed-point application",
            variable.as_str()
        ),
    }
}

/// A worklist of pending equations, charged before it grows.
///
/// One operation is charged per pending pair before the pair is cloned and
/// pushed: every pair will cost at least one pop, so the pending work is paid
/// for up front (F24, PR #384 round 5). That is what keeps an over-budget
/// unification from cloning and enqueueing children without bound.
fn push_pair(
    worklist: &mut Vec<(Term, Term)>,
    left: &Term,
    right: &Term,
    resources: &mut RelationResources,
) -> RewriteResult<()> {
    resources.record_operations(1)?;
    // Admitted: `left`/`right` are the input roots or children of an admitted
    // term, so their shape is inside the relation ceilings and the clone is
    // bounded.
    worklist.push((left.clone(), right.clone()));
    Ok(())
}

/// The cache of equation pairs whose resolution is still stable.
///
/// A remembered pair is only ever reused while the substitution is unchanged:
/// [`Self::invalidate`] runs on EVERY binding mutation, and the remembered
/// decision is a pure function of the pair and the substitution. The key is the
/// popped pair itself, i.e. the identity of the stable resolution: a key derived
/// from the *resolved* pair could only be computed after paying the resolution
/// walk the cache exists to avoid. The key is the worklist's own pair, moved in
/// rather than cloned, so remembering costs no allocation beyond the pair the
/// worklist already owned; identical pairs collapse (one key), which is what
/// keeps the memory bounded by the distinct equations actually seen.
struct SeenEquations {
    seen: BTreeSet<(Term, Term)>,
}

impl SeenEquations {
    fn new() -> Self {
        Self {
            seen: BTreeSet::new(),
        }
    }

    /// Was this pair already processed under the current substitution?
    fn contains(&self, pair: &(Term, Term)) -> bool {
        self.seen.contains(pair)
    }

    /// Remember a decided pair. Returns `true` when it was new.
    fn remember(&mut self, pair: (Term, Term)) -> bool {
        self.seen.insert(pair)
    }

    /// Forget every remembered pair. Called on EVERY binding mutation, so a
    /// remembered pair is only ever reused while the substitution is unchanged.
    fn invalidate(&mut self) {
        self.seen.clear();
    }
}

/// Measure both borrowed inputs and every pre-existing range, then admit the
/// binding graph (F24/F23, PR #384 round 5).
///
/// All walks are iterative and charged to the caller's meter; none clones. The
/// order is: input shapes, then ranges, then the cycle walk.
fn admit_borrowed_inputs(
    substitution: &Substitution,
    left: &Term,
    right: &Term,
    resources: &mut RelationResources,
) -> RewriteResult<()> {
    for term in [left, right] {
        let shape = measure_borrowed(term, resources)?;
        resources.record_term(shape.nodes, shape.depth)?;
    }
    for (_, range) in substitution.iter() {
        let shape = measure_borrowed(range, resources)?;
        resources.record_term(shape.nodes, shape.depth)?;
    }
    admit_acyclic(substitution, resources)
}

/// Admit the pre-existing binding graph only if it is acyclic, with the single
/// exception of a direct identity binding `X |-> X`.
///
/// Explicit stack, one operation per inspected binding: no recursion, so a
/// caller-supplied chain cannot exhaust the call stack. A back edge to a
/// variable still on the walk is a cycle; it is supported only when the edge is
/// `X |-> X` itself, because that is the one cycle whose fixed-point
/// application keeps both sides equal.
fn admit_acyclic(
    substitution: &Substitution,
    resources: &mut RelationResources,
) -> RewriteResult<()> {
    enum Frame<'a> {
        Enter(&'a Variable, Option<&'a Variable>),
        Leave(&'a Variable),
    }
    let mut active: BTreeSet<&Variable> = BTreeSet::new();
    let mut done: BTreeSet<&Variable> = BTreeSet::new();
    for (start, _) in substitution.iter() {
        if done.contains(start) {
            continue;
        }
        let mut stack: Vec<Frame<'_>> = vec![Frame::Enter(start, None)];
        while let Some(frame) = stack.pop() {
            match frame {
                Frame::Leave(variable) => {
                    let _ = active.remove(variable);
                    let _ = done.insert(variable);
                }
                Frame::Enter(variable, parent) => {
                    resources.record_operations(1)?;
                    if done.contains(variable) {
                        continue;
                    }
                    if active.contains(variable) {
                        // A cycle: the only supported one is the identity self
                        // binding `X |-> X`, and it is a self edge by
                        // construction (its only range occurrence is `X`).
                        if parent == Some(variable) && is_identity_binding(substitution, variable) {
                            continue;
                        }
                        return Err(cyclic(variable));
                    }
                    let _ = active.insert(variable);
                    stack.push(Frame::Leave(variable));
                    let Some(range) = substitution.get_var(variable) else {
                        continue;
                    };
                    let mut pending: Vec<&Term> = vec![range];
                    while let Some(node) = pending.pop() {
                        match node {
                            Term::Var(next) => stack.push(Frame::Enter(next, Some(variable))),
                            Term::Sym(_, args) => pending.extend(args.iter()),
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

/// Resolve both sides of one equation and decide what to do with it.
///
/// Resolution is a borrowed root chase: it returns a reference into the
/// equation or into the substitution and allocates nothing. Clones happen only
/// for the decision's own ownership requirement, and only after admission.
fn resolve_pair(
    substitution: &Substitution,
    lhs: &Term,
    rhs: &Term,
    resources: &mut RelationResources,
) -> RewriteResult<Decision> {
    let left = resolve(substitution, lhs, resources)?;
    let right = resolve(substitution, rhs, resources)?;
    Ok(match (left, right) {
        (Term::Var(left), Term::Var(right)) if left == right => Decision::Equal,
        (Term::Var(variable), term) => Decision::Bind(variable.clone(), term.clone()),
        (term, Term::Var(variable)) => Decision::Bind(variable.clone(), term.clone()),
        (Term::Sym(left_head, left_args), Term::Sym(right_head, right_args)) => {
            if left_head != right_head {
                return Err(failure("head symbol clash"));
            }
            if left_args.len() != right_args.len() {
                return Err(failure("arity mismatch"));
            }
            let children = left_args
                .iter()
                .zip(right_args.iter())
                .map(|(left_arg, right_arg)| (left_arg.clone(), right_arg.clone()))
                .collect();
            Decision::Children(children)
        }
    })
}

/// Resolve a term's ROOT through the substitution, iteratively.
///
/// The returned reference is either a subterm of `term` or a borrowed binding
/// range; nothing is cloned and no expansion is materialized. Only the root is
/// resolved (as before): the arguments of a resolved symbol are inspected by
/// [`occurs`] and by the caller's worklist, which is what keeps binding ranges
/// verbatim.
///
/// A variable that re-enters its own resolution is a fixed point. Only the
/// identity binding `X |-> X` is admitted here ([`admit_acyclic`] rejects every
/// other cycle before this walk runs), so the re-entry is returned in place;
/// each binding on the chain is charged one operation.
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
            // Fixed point: only the identity binding is admitted (defence in
            // depth — `admit_acyclic` already rejected every other cycle).
            if !is_identity_binding(substitution, variable) {
                return Err(cyclic(variable));
            }
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
    // `resolve` guarantees the variable is unbound (or identity-bound) here, and
    // the term is resolved, so insertion preserves idempotence.
    substitution.insert(variable.clone(), term);
    Ok(())
}

/// Is `variable` bound to itself (`X |-> X`)?
///
/// The one supported cycle: application leaves the variable in place at its own
/// fixed point, so both occurrences denote the same term (F23).
fn is_identity_binding(substitution: &Substitution, variable: &Variable) -> bool {
    matches!(
        substitution.get_var(variable),
        Some(Term::Var(bound)) if bound == variable
    )
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relation::RelationLimits;

    fn leaf_pair(left: &str, right: &str) -> (Term, Term) {
        (Term::constant(left), Term::constant(right))
    }

    /// F25 (P2): a remembered equation is forgotten by EVERY binding mutation.
    #[test]
    fn a_binding_mutation_invalidates_every_remembered_equation() {
        let mut seen = SeenEquations::new();
        let pair = leaf_pair("c", "c");
        assert!(!seen.contains(&pair), "first sighting is new");
        assert!(seen.remember(pair.clone()), "the pair is remembered once");
        assert!(seen.contains(&pair), "second sighting is a hit");
        assert!(
            !seen.remember(pair.clone()),
            "a repeated sighting adds no key"
        );
        seen.invalidate();
        assert!(
            !seen.contains(&pair),
            "after a binding mutation the same pair must be re-evaluated"
        );
    }

    /// A pair with children is remembered too: its children would otherwise be
    /// re-pushed on every duplicate. The key is the pair the worklist owned, so
    /// remembering allocates nothing extra.
    #[test]
    fn a_decomposed_pair_is_remembered_until_a_mutation() {
        let mut seen = SeenEquations::new();
        let pair = (
            Term::sym("f", vec![Term::var("X")]),
            Term::sym("f", vec![Term::var("X")]),
        );
        assert!(!seen.contains(&pair));
        assert!(seen.remember(pair.clone()));
        assert!(seen.contains(&pair));
        seen.invalidate();
        assert!(!seen.contains(&pair));
    }

    /// F23 (P1): the admission walk is iterative, so a deep binding chain is a
    /// typed rejection instead of a stack overflow.
    #[test]
    fn a_deep_binding_chain_is_rejected_by_the_meter_not_the_stack() {
        let mut substitution = Substitution::new();
        let depth = 4_000;
        for index in 0..depth {
            substitution.insert(format!("X{index}"), Term::var(format!("X{}", index + 1)));
        }
        substitution.insert(format!("X{depth}"), Term::constant("a"));
        // Admitted: acyclic, so the unification of the chain ends at `a`.
        let mut resources = RelationResources::new(&RelationLimits::default());
        unify_with(
            &mut substitution,
            &Term::var("X0"),
            &Term::var("Y"),
            &mut resources,
        )
        .expect("an acyclic chain is admitted");
        assert_eq!(substitution.get("Y"), Some(&Term::constant("a")));
        // The same chain under a tiny operation budget is a typed rejection of
        // the admission walk, not a stack overflow in a clone.
        let limits = RelationLimits::new(4_096, 64, 4_096, 16).expect("valid limits");
        let mut tiny = RelationResources::new(&limits);
        let error = unify_with(
            &mut substitution,
            &Term::var("X0"),
            &Term::var("Z"),
            &mut tiny,
        )
        .expect_err("the admission walk cannot be paid for");
        assert!(matches!(
            error,
            RewriteError::RelationLimitExceeded {
                resource: "operations",
                ..
            }
        ));
    }
}
