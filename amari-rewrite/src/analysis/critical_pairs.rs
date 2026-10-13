// SPDX-License-Identifier: MIT OR Apache-2.0

//! Deterministic critical pairs for first-order rewrite systems.
//!
//! Terminology follows Baader–Nipkow, *Term Rewriting and All That*,
//! chapter 6: a critical pair is generated when the left-hand side of
//! one rule overlaps a non-variable subterm of another rule's
//! left-hand side, provided the two patterns unify after the second
//! rule is renamed apart. Only unification *failure* is a non-event;
//! relation-resource exhaustion is a typed error and aborts without a
//! partial report.
//!
//! Enumeration is total and deterministic: left rule index ascending,
//! then right rule index ascending, then the overlap position in
//! preorder. To match the textbook convention — and to avoid emitting
//! each critical pair twice in mirrored form — a *root* overlap
//! between two distinct rules is generated only once, for
//! `left_rule < right_rule`; non-root overlaps over all ordered rule
//! pairs, and self-overlaps only at non-root positions.

use alloc::string::String;
use alloc::vec::Vec;

use crate::analysis::unify::{term_depth, term_nodes, unify};
use crate::error::{RewriteError, RewriteResult};
use crate::relation::RelationResources;
use crate::trs::{Rule, Substitution, Term, TermSystem};
use crate::Path;

/// One overlap of `right_rule` renamed apart and unified into
/// `left_rule`'s left-hand side at a non-variable position.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub struct CriticalPair {
    /// Index of the overlapped rule (the outer rule).
    pub left_rule: usize,
    /// Index of the rule unified into the position (the inner rule).
    pub right_rule: usize,
    /// Overlap position inside the left rule's LHS.
    pub path: Path,
    /// The unified left LHS `l1σ`.
    pub overlap: Term,
    /// `l1` rewritten at the root: `r1σ`.
    pub root_step: Term,
    /// `l1` rewritten at `path`: `l1σ[r2σ]_p`.
    pub position_step: Term,
}

/// Report over every critical pair of a rewrite system.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub struct CriticalPairReport {
    /// Generated pairs in deterministic enumeration order.
    pub pairs: Vec<CriticalPair>,
    /// Overlaps that unified but whose two sides are structurally
    /// equal, so no pair was emitted.
    pub trivial_excluded: usize,
    /// Indices of rules whose LHS is not left-linear. Recorded for the
    /// confluence classifier; pairs are still generated for them.
    pub non_left_linear_rules: Vec<usize>,
}

/// Enumerate every critical pair of `system`.
///
/// One operation is accounted per overlap attempt (before unification,
/// which meters itself); one constraint is accounted per emitted or
/// trivially excluded pair. Exhausting either budget returns
/// [`RewriteError::RelationLimitExceeded`] without a partial report.
pub fn critical_pairs(
    system: &TermSystem,
    resources: &mut RelationResources,
) -> RewriteResult<CriticalPairReport> {
    let rules = system.rules();
    let mut non_left_linear_rules = Vec::new();
    for (index, rule) in rules.iter().enumerate() {
        // Validate every input term and meter preprocessing BEFORE
        // any enumeration state is built (PR #301 round 1): every
        // later traversal and construction operates on bounded terms.
        resources.record_operations(1)?;
        resources.record_term(term_nodes(rule.lhs()), term_depth(rule.lhs()))?;
        resources.record_term(term_nodes(rule.rhs()), term_depth(rule.rhs()))?;
        if !rule.is_left_linear() {
            non_left_linear_rules.push(index);
        }
    }

    let mut pairs = Vec::new();
    let mut trivial_excluded = 0usize;

    for (left_rule, left) in rules.iter().enumerate() {
        let left_lhs = left.lhs();
        let prefix = fresh_prefix(left_lhs);
        let positions = left_lhs.positions();
        for (right_rule, right) in rules.iter().enumerate() {
            for path in &positions {
                let Some(subterm) = left_lhs.subterm(path) else {
                    continue;
                };
                if subterm.is_var() {
                    continue;
                }
                if path.is_root() {
                    // Self-overlaps never use the root, and a root
                    // overlap between distinct rules is emitted once
                    // (mirrored duplicates are not critical pairs).
                    if left_rule >= right_rule {
                        continue;
                    }
                }

                resources.record_operations(1)?;
                let renamed = rename_rule_variables(right, &prefix);
                let subst = match unify(subterm, renamed.lhs(), resources) {
                    Ok(subst) => subst,
                    Err(RewriteError::UnificationFailure { .. }) => continue,
                    Err(other) => return Err(other),
                };

                // Bounds before allocation (PR #301 round 1): the
                // exact shape of every evidence term is computed from
                // the templates and substitution ranges, enforced
                // against the term limits, and the evidence cell is
                // reserved BEFORE anything is materialized.
                let (overlap_nodes, overlap_depth) = applied_shape(left_lhs, &subst, None);
                let (root_nodes, root_depth) = applied_shape(left.rhs(), &subst, None);
                let (inner_nodes, inner_depth) = applied_shape(renamed.rhs(), &subst, None);
                let (spine_nodes, spine_depth) = applied_shape(left_lhs, &subst, Some(path));
                let position_nodes = spine_nodes + inner_nodes;
                let position_depth = spine_depth.max(path.as_slice().len() + inner_depth);
                resources.record_term(overlap_nodes, overlap_depth)?;
                resources.record_term(root_nodes, root_depth)?;
                resources.record_term(position_nodes, position_depth)?;
                resources.record_constraints(1)?;

                let overlap = subst.apply(left_lhs);
                let root_step = subst.apply(left.rhs());
                // `overlap|path == subst.apply(renamed.lhs())` by
                // construction, so the replacement position exists.
                let position_step = overlap.replace_at(path, subst.apply(renamed.rhs()))?;

                if root_step == position_step {
                    trivial_excluded += 1;
                    continue;
                }
                pairs.push(CriticalPair {
                    left_rule,
                    right_rule,
                    path: path.clone(),
                    overlap,
                    root_step,
                    position_step,
                });
            }
        }
    }

    // No deduplication pass is needed: every emitted pair carries a
    // distinct `(left_rule, right_rule, path)` provenance triple,
    // and the enumeration visits each triple exactly once.
    Ok(CriticalPairReport {
        pairs,
        trivial_excluded,
        non_left_linear_rules,
    })
}

/// Pick a rename prefix that collides with no left-LHS variable name.
///
/// Starts at `"cpR"` and appends `'#'` until no left variable begins
/// with the prefix, which guarantees the renamed right variables are
/// disjoint from the left rule's variables.
fn fresh_prefix(left_lhs: &Term) -> String {
    let left_variables = left_lhs.variables();
    let mut prefix = String::from("cpR");
    while left_variables
        .iter()
        .any(|variable| variable.as_str().starts_with(&prefix))
    {
        prefix.push('#');
    }
    prefix
}

/// Return `rule` with every variable `v` renamed to `"{prefix}{v}"`.
///
/// One-pass structural renaming (PR #301 round 1): deep substitution
/// through [`Substitution::apply`] re-resolves freshly introduced
/// names, so a rule mentioning `x` and `cpRx` would collapse both to
/// `cpRcpRx` (and further). Building the renamed term directly makes
/// capture impossible by construction. Renaming preserves rule
/// validity (RHS variables remain a subset of LHS variables), so the
/// rule is rebuilt unchecked.
fn rename_rule_variables(rule: &Rule, prefix: &str) -> Rule {
    Rule::new_unchecked(
        rename_variables(rule.lhs(), prefix),
        rename_variables(rule.rhs(), prefix),
    )
}

/// Structural one-pass variable renaming; recursion depth is bounded
/// by the caller's input term-depth validation.
fn rename_variables(term: &Term, prefix: &str) -> Term {
    match term {
        Term::Var(variable) => {
            let mut name = String::with_capacity(prefix.len() + variable.as_str().len());
            name.push_str(prefix);
            name.push_str(variable.as_str());
            Term::var(name)
        }
        Term::Sym(symbol, args) => Term::Sym(
            symbol.clone(),
            args.iter()
                .map(|arg| rename_variables(arg, prefix))
                .collect(),
        ),
    }
}

/// Exact `(nodes, depth)` of `σ(template)` computed from the template
/// and the substitution ranges WITHOUT materializing the application
/// (PR #301 round 1: term limits are enforced before any evidence
/// term is allocated). With `except`, the subtree at that path is
/// skipped; `applied_shape(template, σ, Some(p))` plus the shape of
/// the replacement gives the exact shape of the position step.
fn applied_shape(template: &Term, subst: &Substitution, except: Option<&Path>) -> (usize, usize) {
    struct Frame<'a> {
        term: &'a Term,
        path: Path,
    }
    let mut nodes = 0usize;
    let mut depth = 0usize;
    let mut stack = Vec::from([Frame {
        term: template,
        path: Path::root(),
    }]);
    while let Some(frame) = stack.pop() {
        if let Some(skip) = except {
            if frame.path.as_slice() == skip.as_slice() {
                continue;
            }
        }
        match frame.term {
            Term::Var(variable) => {
                let (bound_nodes, bound_depth) = subst
                    .get_var(variable)
                    .map(|bound| (term_nodes(bound), term_depth(bound)))
                    .unwrap_or((1, 1));
                nodes += bound_nodes;
                depth = depth.max(frame.path.as_slice().len() + bound_depth);
            }
            Term::Sym(_, args) => {
                nodes += 1;
                if args.is_empty() {
                    depth = depth.max(frame.path.as_slice().len() + 1);
                }
                for (index, arg) in args.iter().enumerate() {
                    stack.push(Frame {
                        term: arg,
                        path: frame.path.child(index),
                    });
                }
            }
        }
    }
    (nodes, depth)
}
