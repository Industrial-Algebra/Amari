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

use crate::analysis::unify::unify;
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

                let overlap = subst.apply(left_lhs);
                let root_step = subst.apply(left.rhs());
                // `overlap|path == subst.apply(renamed.lhs())` by
                // construction, so the replacement position exists.
                let position_step = overlap.replace_at(path, subst.apply(renamed.rhs()))?;

                resources.record_constraints(1)?;
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
fn rename_rule_variables(rule: &Rule, prefix: &str) -> Rule {
    let mut subst = Substitution::new();
    for variable in rule.lhs().variables() {
        let mut name = String::from(prefix);
        name.push_str(variable.as_str());
        subst.insert(variable, Term::var(name));
    }
    for variable in rule.rhs().variables() {
        if subst.get_var(&variable).is_none() {
            let mut name = String::from(prefix);
            name.push_str(variable.as_str());
            subst.insert(variable, Term::var(name));
        }
    }
    Rule::new_unchecked(subst.apply(rule.lhs()), subst.apply(rule.rhs()))
}
