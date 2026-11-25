// SPDX-License-Identifier: MIT OR Apache-2.0

//! Bounded substitution application and whole-trace variable correspondence
//! for replay verification.
//!
//! ## Bounded application (F11/F12, PR #384 round 3)
//!
//! Replay applies caller-owned substitutions and rule matchings to terms it
//! only partially controls. Deep application is exponential in the worst case
//! (a chain `X10 |-> pair(X11,X11)`, `X9 |-> pair(X10,X10)`, ... doubles at
//! every step), so measuring the *result* is too late: the expansion has already
//! been materialized. Every application here therefore follows one order:
//!
//! 1. **preflight** — an explicit-worklist walk of the expansion *shape*
//!    (nodes and depth) under the same fixed-point semantics as
//!    [`Substitution::apply`], charging one operation per inspected node and
//!    returning [`RewriteError::RelationLimitExceeded`] as soon as a ceiling is
//!    exceeded. Nothing proportional to the expansion is allocated: the
//!    worklist only ever holds borrowed terms, and it is abandoned at the
//!    ceiling;
//! 2. **charge** — [`RelationResources::record_term`] is applied to the
//!    preflighted shape, so an over-limit term is rejected *before* the
//!    allocation it would need;
//! 3. **materialize** — only a term whose shape already fit the budget is
//!    built, and it is built iteratively (no call-stack depth proportional to a
//!    chained substitution).
//!
//! Cycle semantics follow [`Substitution::apply`] exactly: a binding that
//! re-enters a variable already being resolved leaves that variable in place,
//! so `X |-> X` and `X |-> f(X)` resolve to their fixed point instead of
//! diverging. [`apply_checked`] is pinned against `apply` by unit tests.
//!
//! ## Whole-trace correspondence (F9)
//!
//! A recorded trace is symbolic: its step terms carry freshened variables whose
//! names are independent of the names the replay reaches. What must be
//! consistent is the *renaming*: one bijection from the recorded trace's free
//! variables to the replay's, extended over every record and anchored by the
//! leaf/goal unifier. Comparing recorded terms under independent per-term
//! alpha-canonicalization is weaker — it admits a middle record that is an
//! alpha-variant of the replayed state while sharing a variable name with the
//! anchored goal and meaning a different instance.
//!
//! [`TraceVariables`] carries both directions of that bijection, so a variable
//! already mapped to one counterpart cannot be re-mapped and two recorded
//! variables cannot collapse onto one replay variable.

use alloc::collections::{BTreeMap, BTreeSet, VecDeque};
use alloc::vec::Vec;

use crate::error::{RewriteError, RewriteResult};
use crate::relation::RelationResources;
use crate::trs::{Substitution, Symbol, Term, Variable};

/// Node and depth shape of a term, at the granularity the relation ceilings
/// enforce.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TermShape {
    /// Number of nodes (variables and symbols).
    pub(crate) nodes: usize,
    /// Depth, counting a leaf as one.
    pub(crate) depth: usize,
}

fn limit(resource: &'static str, ceiling: usize) -> RewriteError {
    RewriteError::RelationLimitExceeded {
        resource,
        limit: ceiling,
    }
}

/// A borrowed measurement rejection with the observed value preserved.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum BorrowedMeasurementError {
    /// A term shape exceeded its ceiling.
    Shape {
        resource: &'static str,
        value: usize,
        limit: usize,
    },
    /// The shared operation meter was exhausted.
    Operations { limit: usize },
}

impl BorrowedMeasurementError {
    fn into_rewrite(self) -> RewriteError {
        match self {
            Self::Shape {
                resource, limit, ..
            } => RewriteError::RelationLimitExceeded { resource, limit },
            Self::Operations { limit } => RewriteError::RelationLimitExceeded {
                resource: "operations",
                limit,
            },
        }
    }
}

/// Preflight a borrowed term's shape, preserving the offending lower bound.
///
/// Iterative (explicit worklist), charging one operation per inspected node
/// and rejecting before an unaffordable queue grows.
pub(crate) fn measure_borrowed_detailed(
    term: &Term,
    resources: &mut RelationResources,
) -> Result<TermShape, BorrowedMeasurementError> {
    let max_nodes = resources.max_term_nodes();
    let max_depth = resources.max_term_depth();
    let mut nodes = 0usize;
    let mut depth = 1usize;
    let mut stack: VecDeque<(&Term, usize)> = VecDeque::new();
    stack.push_back((term, 1));
    while let Some((node, node_depth)) = stack.pop_front() {
        resources
            .record_operations(1)
            .map_err(|_| BorrowedMeasurementError::Operations {
                limit: resources.max_operations(),
            })?;
        nodes = nodes.saturating_add(1);
        if nodes > max_nodes {
            return Err(BorrowedMeasurementError::Shape {
                resource: "term nodes",
                value: nodes,
                limit: max_nodes,
            });
        }
        if node_depth > max_depth {
            return Err(BorrowedMeasurementError::Shape {
                resource: "term depth",
                value: node_depth,
                limit: max_depth,
            });
        }
        depth = depth.max(node_depth);
        if let Term::Sym(_, args) = node {
            let pending = nodes.saturating_add(stack.len()).saturating_add(args.len());
            if pending > max_nodes {
                return Err(BorrowedMeasurementError::Shape {
                    resource: "term nodes",
                    value: pending,
                    limit: max_nodes,
                });
            }
            let inevitable = stack.len().saturating_add(args.len());
            if inevitable > resources.remaining_operations() {
                return Err(BorrowedMeasurementError::Operations {
                    limit: resources.max_operations(),
                });
            }
            for arg in args {
                stack.push_back((arg, node_depth + 1));
            }
        }
    }
    Ok(TermShape { nodes, depth })
}

/// Preflight a borrowed term's shape under the ordinary rewrite error type.
pub(crate) fn measure_borrowed(
    term: &Term,
    resources: &mut RelationResources,
) -> RewriteResult<TermShape> {
    measure_borrowed_detailed(term, resources).map_err(BorrowedMeasurementError::into_rewrite)
}

/// Measure the shape `term` would have after this substitution is applied.
///
/// Deep application with [`Substitution::apply`]'s chain resolution and
/// fixed-point cycle semantics, measured without materializing anything.
///
/// The worklist holds two kinds of frame: an ENTER frame (a node that will
/// appear in the result, or resolve into one) and a LEAVE frame (bookkeeping
/// that pops a variable from the cycle guard). Every pending Enter frame —
/// whether it holds a symbol or a variable — is guaranteed to contribute at
/// least one output node: an unbound variable is its own output, a bound
/// variable is replaced by a range of at least one node, and a symbol is its
/// own output plus its children. Only Leave frames are bookkeeping, so the
/// charge-before-push lower bound counts `nodes + pending_enters + args.len()`
/// and never a cleanup frame.
///
/// Counting only pending SYMBOL frames (the round-4 formula) undercounted a
/// chained application by every pending variable: `X0 |-> f(X1, a*3999)`,
/// `X1 |-> f(a*4000)` reports 4,002 inevitable nodes at a point where 8,001 are
/// inevitable, and pushed 4,000 further children before rejecting (F29, PR #384
/// round 5). Counting every Enter frame is still a lower bound, so an expansion
/// of exactly the ceiling stays admissible (F17).
fn measure_apply(
    substitution: &Substitution,
    term: &Term,
    resources: &mut RelationResources,
) -> RewriteResult<TermShape> {
    enum Frame<'a> {
        Enter(&'a Term, usize),
        Leave(&'a Variable),
    }
    let max_nodes = resources.max_term_nodes();
    let max_depth = resources.max_term_depth();
    let mut visiting: BTreeSet<&Variable> = BTreeSet::new();
    let mut nodes = 0usize;
    let mut depth = 1usize;
    let mut stack: Vec<Frame<'_>> = Vec::from([Frame::Enter(term, 1)]);
    // Pending Enter frames on the stack, each of which is guaranteed to
    // contribute one output node. Leave frames are bookkeeping only.
    let mut pending_enters = 1usize;
    while let Some(frame) = stack.pop() {
        match frame {
            Frame::Leave(variable) => {
                let _ = visiting.remove(variable);
            }
            Frame::Enter(node, node_depth) => {
                pending_enters = pending_enters.saturating_sub(1);
                resources.record_operations(1)?;
                match node {
                    // A variable being resolved is replaced by its range, so it
                    // contributes no node of its own and does not add depth.
                    // Re-entering a variable already being resolved is the
                    // fixed point: the variable is left in place.
                    Term::Var(variable) if !visiting.contains(variable) => {
                        match substitution.get_var(variable) {
                            Some(bound) => {
                                let _ = visiting.insert(variable);
                                stack.push(Frame::Leave(variable));
                                stack.push(Frame::Enter(bound, node_depth));
                                pending_enters = pending_enters.saturating_add(1);
                            }
                            None => {
                                nodes = nodes.saturating_add(1);
                                if nodes > max_nodes {
                                    return Err(limit("term nodes", max_nodes));
                                }
                                if node_depth > max_depth {
                                    return Err(limit("term depth", max_depth));
                                }
                                depth = depth.max(node_depth);
                            }
                        }
                    }
                    Term::Var(_) => {
                        nodes = nodes.saturating_add(1);
                        if nodes > max_nodes {
                            return Err(limit("term nodes", max_nodes));
                        }
                        if node_depth > max_depth {
                            return Err(limit("term depth", max_depth));
                        }
                        depth = depth.max(node_depth);
                    }
                    Term::Sym(_, args) => {
                        nodes = nodes.saturating_add(1);
                        if nodes > max_nodes {
                            return Err(limit("term nodes", max_nodes));
                        }
                        if node_depth > max_depth {
                            return Err(limit("term depth", max_depth));
                        }
                        depth = depth.max(node_depth);
                        // Charge-before-push: every pending Enter frame and
                        // every child about to be pushed is one output node, so
                        // an over-budget expansion is rejected before the
                        // worklist grows to its arity (F29). Cleanup leaves are
                        // not output nodes, so an expansion of exactly the
                        // ceiling is admissible (F17).
                        let pending = nodes
                            .saturating_add(pending_enters)
                            .saturating_add(args.len());
                        if pending > max_nodes {
                            return Err(limit("term nodes", max_nodes));
                        }
                        for arg in args.iter().rev() {
                            stack.push(Frame::Enter(arg, node_depth + 1));
                            pending_enters = pending_enters.saturating_add(1);
                        }
                    }
                }
            }
        }
    }
    Ok(TermShape { nodes, depth })
}

/// Apply a substitution to a term under the relation budget.
///
/// See the module documentation for the preflight-then-charge-then-materialize
/// order. The result is exactly [`Substitution::apply`]'s value; only the
/// rejection threshold moves (before the allocation).
pub(crate) fn apply_checked(
    substitution: &Substitution,
    term: &Term,
    resources: &mut RelationResources,
) -> RewriteResult<Term> {
    let shape = measure_apply(substitution, term, resources)?;
    resources.record_term(shape.nodes, shape.depth)?;
    Ok(build_apply(substitution, term))
}

/// Materialize a substitution application that has already been preflighted.
fn build_apply(substitution: &Substitution, term: &Term) -> Term {
    enum Frame<'a> {
        Enter(&'a Term),
        Leave(&'a Variable),
        Build(&'a Symbol, usize),
    }
    let mut visiting: BTreeSet<&Variable> = BTreeSet::new();
    let mut built: Vec<Term> = Vec::new();
    let mut stack: Vec<Frame<'_>> = Vec::from([Frame::Enter(term)]);
    while let Some(frame) = stack.pop() {
        match frame {
            Frame::Leave(variable) => {
                let _ = visiting.remove(variable);
            }
            Frame::Build(symbol, arity) => {
                let start = built.len() - arity;
                let args = built.split_off(start);
                built.push(Term::Sym(symbol.clone(), args));
            }
            Frame::Enter(node) => match node {
                Term::Var(variable) if !visiting.contains(variable) => {
                    match substitution.get_var(variable) {
                        Some(bound) => {
                            let _ = visiting.insert(variable);
                            stack.push(Frame::Leave(variable));
                            stack.push(Frame::Enter(bound));
                        }
                        None => built.push(Term::Var(variable.clone())),
                    }
                }
                Term::Var(variable) => built.push(Term::Var(variable.clone())),
                Term::Sym(symbol, args) => {
                    stack.push(Frame::Build(symbol, args.len()));
                    for arg in args.iter().rev() {
                        stack.push(Frame::Enter(arg));
                    }
                }
            },
        }
    }
    built
        .pop()
        .expect("a preflighted application builds one term")
}

/// Measure the shape `term` would have after a MATCHER substitution is applied.
///
/// Matcher ranges are verbatim data: [`Substitution::apply_matcher`] never
/// re-resolves a variable inside a binding, so a range is measured as written
/// and its internal variables are leaves. That is what makes the measurement
/// finite for self-referential matcher bindings (`X |-> X`).
pub(crate) fn measure_matcher(
    substitution: &Substitution,
    term: &Term,
    resources: &mut RelationResources,
) -> RewriteResult<TermShape> {
    enum Frame<'a> {
        Enter(&'a Term, usize, bool),
    }
    let max_nodes = resources.max_term_nodes();
    let max_depth = resources.max_term_depth();
    let mut nodes = 0usize;
    let mut depth = 1usize;
    let mut stack: Vec<Frame<'_>> = Vec::from([Frame::Enter(term, 1, true)]);
    while let Some(Frame::Enter(node, node_depth, resolve)) = stack.pop() {
        resources.record_operations(1)?;
        match node {
            // A resolving variable is replaced by its (verbatim) range, which
            // is measured at the variable's own depth and WITHOUT further
            // resolution.
            Term::Var(variable) if resolve => match substitution.get_var(variable) {
                Some(bound) => stack.push(Frame::Enter(bound, node_depth, false)),
                None => {
                    nodes = nodes.saturating_add(1);
                    if nodes > max_nodes {
                        return Err(limit("term nodes", max_nodes));
                    }
                    if node_depth > max_depth {
                        return Err(limit("term depth", max_depth));
                    }
                    depth = depth.max(node_depth);
                }
            },
            Term::Var(_) => {
                nodes = nodes.saturating_add(1);
                if nodes > max_nodes {
                    return Err(limit("term nodes", max_nodes));
                }
                if node_depth > max_depth {
                    return Err(limit("term depth", max_depth));
                }
                depth = depth.max(node_depth);
            }
            Term::Sym(_, args) => {
                nodes = nodes.saturating_add(1);
                if nodes > max_nodes {
                    return Err(limit("term nodes", max_nodes));
                }
                if node_depth > max_depth {
                    return Err(limit("term depth", max_depth));
                }
                depth = depth.max(node_depth);
                let pending = nodes.saturating_add(stack.len()).saturating_add(args.len());
                if pending > max_nodes {
                    return Err(limit("term nodes", max_nodes));
                }
                for arg in args.iter().rev() {
                    stack.push(Frame::Enter(arg, node_depth + 1, resolve));
                }
            }
        }
    }
    Ok(TermShape { nodes, depth })
}

/// The shape of `term` after the subterm at `path` is replaced.
///
/// The node count is arithmetic on the preflighted shapes; the depth is the
/// deepest sibling on the path against the replacement's depth. Nothing is
/// allocated and the walk is bounded by the already-admitted term.
pub(crate) fn shape_after_replacement(
    term: &Term,
    path: &[usize],
    current: TermShape,
    subterm: TermShape,
    replacement: TermShape,
    resources: &mut RelationResources,
) -> RewriteResult<TermShape> {
    let nodes = current
        .nodes
        .saturating_sub(subterm.nodes)
        .saturating_add(replacement.nodes);
    let depth = replaced_depth(term, path, replacement.depth, resources)?;
    Ok(TermShape { nodes, depth })
}

fn replaced_depth(
    term: &Term,
    path: &[usize],
    replacement_depth: usize,
    resources: &mut RelationResources,
) -> RewriteResult<usize> {
    let Some((index, rest)) = path.split_first() else {
        return Ok(replacement_depth);
    };
    let Term::Sym(_, args) = term else {
        return Err(RewriteError::InvalidPath);
    };
    let child = args.get(*index).ok_or(RewriteError::InvalidPath)?;
    let mut depth = replaced_depth(child, rest, replacement_depth, resources)?;
    for (position, sibling) in args.iter().enumerate() {
        if position == *index {
            continue;
        }
        depth = depth.max(measure_borrowed(sibling, resources)?.depth);
    }
    Ok(depth.saturating_add(1))
}

/// One whole-trace free-variable correspondence: alpha equivalence between the
/// recorded trace's free variables and the replay's.
///
/// Both directions are kept, so the relation is a bijection: a recorded
/// variable already mapped to one replay variable cannot be re-mapped, and two
/// recorded variables cannot collapse onto the same replay variable. Extension
/// happens in place and has no rollback (like [`crate::trs::match_pattern`]'s
/// accumulating sibling), so callers treat `false` as terminal.
#[derive(Clone, Debug, Default)]
pub(crate) struct TraceVariables {
    recorded_to_replay: BTreeMap<Variable, Variable>,
    replay_to_recorded: BTreeMap<Variable, Variable>,
}

impl TraceVariables {
    /// An empty correspondence.
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Extend the correspondence so `recorded` and `replay` are alpha
    /// equivalent. Returns `false` on a structural clash or a conflicting
    /// renaming, without rolling back the extension already made.
    pub(crate) fn match_alpha(&mut self, recorded: &Term, replay: &Term) -> bool {
        let mut stack: Vec<(&Term, &Term)> = Vec::from([(recorded, replay)]);
        while let Some((recorded_node, replay_node)) = stack.pop() {
            match (recorded_node, replay_node) {
                (Term::Var(recorded_var), Term::Var(replay_var)) => {
                    match (
                        self.recorded_to_replay.get(recorded_var),
                        self.replay_to_recorded.get(replay_var),
                    ) {
                        (Some(mapped), _) => {
                            if mapped != replay_var {
                                return false;
                            }
                        }
                        (None, Some(mapped)) => {
                            if mapped != recorded_var {
                                return false;
                            }
                        }
                        (None, None) => {
                            let _ = self
                                .recorded_to_replay
                                .insert(recorded_var.clone(), replay_var.clone());
                            let _ = self
                                .replay_to_recorded
                                .insert(replay_var.clone(), recorded_var.clone());
                        }
                    }
                }
                (
                    Term::Sym(recorded_symbol, recorded_args),
                    Term::Sym(replay_symbol, replay_args),
                ) => {
                    if recorded_symbol != replay_symbol || recorded_args.len() != replay_args.len()
                    {
                        return false;
                    }
                    for (recorded_arg, replay_arg) in recorded_args.iter().zip(replay_args) {
                        stack.push((recorded_arg, replay_arg));
                    }
                }
                _ => return false,
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relation::RelationLimits;
    use alloc::format;
    use alloc::vec;

    fn var(name: &str) -> Term {
        Term::var(name)
    }

    fn pair(left: Term, right: Term) -> Term {
        Term::sym("pair", vec![left, right])
    }

    fn budget() -> RelationResources {
        RelationResources::new(&RelationLimits::default())
    }

    #[test]
    fn checked_application_matches_deep_application() {
        // Chained bindings resolve fully.
        let mut chain = Substitution::new();
        chain.insert("X", var("Y"));
        chain.insert("Y", Term::constant("a"));
        let term = Term::sym("f", vec![var("X"), var("Z")]);
        assert_eq!(
            apply_checked(&chain, &term, &mut budget()).unwrap(),
            chain.apply(&term)
        );
        // Self-referential bindings resolve to the fixed point.
        let mut cyclic = Substitution::new();
        cyclic.insert("X", var("X"));
        cyclic.insert("W", Term::sym("g", vec![var("W")]));
        for term in [var("X"), var("W"), Term::sym("h", vec![var("W")])] {
            assert_eq!(
                apply_checked(&cyclic, &term, &mut budget()).unwrap(),
                cyclic.apply(&term)
            );
        }
        // An unbound variable stays itself.
        assert_eq!(
            apply_checked(&Substitution::new(), &var("Q"), &mut budget()).unwrap(),
            var("Q")
        );
    }

    #[test]
    fn an_exponential_chain_is_rejected_without_materializing_it() {
        let mut substitution = Substitution::new();
        substitution.insert("X11", Term::constant("a"));
        for index in (0..11).rev() {
            let next = Term::var(format!("X{}", index + 1));
            substitution.insert(format!("X{index}"), pair(next.clone(), next));
        }
        let leaf = Term::sym(
            "f",
            (0..12).rev().map(|index| Term::var(format!("X{index}"))),
        );
        let mut resources = budget();
        let error = apply_checked(&substitution, &leaf, &mut resources)
            .expect_err("the expansion exceeds the node ceiling");
        assert!(matches!(
            error,
            RewriteError::RelationLimitExceeded {
                resource: "term nodes",
                ..
            }
        ));
        // The preflight abandoned the walk at the ceiling: the operation meter
        // is bounded by the ceiling (one node charge per visited node, plus one
        // charge per resolved variable), never by the expansion's full size.
        assert!(
            resources.operations() <= 2 * RelationLimits::MAX_TERM_NODES + 64,
            "the preflight charges at most the ceiling, got {} operations",
            resources.operations()
        );
    }

    #[test]
    fn matcher_application_is_measured_verbatim() {
        let mut matcher = Substitution::new();
        matcher.insert("X", Term::sym("g", vec![var("X")]));
        let term = Term::sym("f", vec![var("X"), var("Y")]);
        // A self-referential matcher binding is data: measured as written, so
        // the measurement terminates and matches the materialized shape.
        let mut resources = budget();
        let shape = measure_matcher(&matcher, &term, &mut resources).unwrap();
        let built = matcher.apply_matcher(&term);
        let measured = measure_borrowed(&built, &mut budget()).unwrap();
        assert_eq!(shape, measured);
    }

    #[test]
    fn replacement_shape_is_exact() {
        let term = Term::sym(
            "f",
            vec![
                Term::constant("a"),
                Term::sym("g", vec![Term::constant("b")]),
            ],
        );
        let current = measure_borrowed(&term, &mut budget()).unwrap();
        let subterm =
            measure_borrowed(term.subterm(&[1usize].into()).unwrap(), &mut budget()).unwrap();
        let replacement = measure_borrowed(
            &pair(Term::constant("c"), Term::constant("d")),
            &mut budget(),
        )
        .unwrap();
        let predicted =
            shape_after_replacement(&term, &[1], current, subterm, replacement, &mut budget())
                .unwrap();
        let rebuilt = term
            .replace_at(
                &[1usize].into(),
                pair(Term::constant("c"), Term::constant("d")),
            )
            .unwrap();
        let actual = measure_borrowed(&rebuilt, &mut budget()).unwrap();
        assert_eq!(predicted, actual);
    }

    #[test]
    fn whole_trace_correspondence_is_bijective() {
        let mut correspondence = TraceVariables::new();
        assert!(correspondence.match_alpha(
            &Term::sym("f", vec![var("X"), var("Y")]),
            &Term::sym("f", vec![var("P"), var("Q")]),
        ));
        // A consistent continuation is accepted.
        assert!(correspondence.match_alpha(
            &Term::sym("g", vec![var("Y"), var("X")]),
            &Term::sym("g", vec![var("Q"), var("P")]),
        ));
        // A conflicting renaming is rejected.
        assert!(!correspondence.match_alpha(
            &Term::sym("g", vec![var("Y"), var("X")]),
            &Term::sym("g", vec![var("P"), var("Q")]),
        ));
        // Two recorded variables may not collapse onto one replay variable.
        let mut collapsing = TraceVariables::new();
        assert!(!collapsing.match_alpha(
            &Term::sym("f", vec![var("X"), var("Y")]),
            &Term::sym("f", vec![var("P"), var("P")]),
        ));
        // Structure must agree.
        let mut structural = TraceVariables::new();
        assert!(!structural.match_alpha(&var("X"), &Term::constant("a")));
    }
}
