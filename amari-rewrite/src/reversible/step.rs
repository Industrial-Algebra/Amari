// SPDX-License-Identifier: MIT OR Apache-2.0

//! Concrete forward steps carrying typed residual authority.

use alloc::string::ToString;

use crate::error::{RewriteError, RewriteResult};
use crate::relation::{
    BackwardClause, LogicVar, RelationLimits, RelationResources, RuleId, Sha256Digest,
};
use crate::reversible::RewriteResidual;
use crate::rewritable::Path;
use crate::trs::{match_pattern, Rule, Term, TermSystem, Variable};

/// The local transition at one position: which rule fired and the
/// subterm before/after.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub struct ForwardTransition {
    /// Identity of the checked rule that fired.
    pub rule_id: RuleId,
    /// Position of the replaced subterm.
    pub position: Path,
    /// The matched subterm before the step.
    pub from: Term,
    /// The instantiated subterm after the step.
    pub to: Term,
}

/// A concrete forward step together with the authority needed to
/// reverse it exactly.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub struct ReversibleStep {
    /// The rewritten term.
    pub target: Term,
    /// Typed residual authority for exact backward replay.
    pub residual: RewriteResidual,
    /// The local transition performed.
    pub transition: ForwardTransition,
}

impl ReversibleStep {
    /// Apply `rule` at `position` in `source`, emitting residual
    /// authority. Fails when the path is invalid or the rule's LHS
    /// does not match there.
    pub fn apply(
        system: &TermSystem,
        source: &Term,
        rule: &Rule,
        position: &Path,
        limits: &RelationLimits,
    ) -> RewriteResult<Self> {
        let _ = system; // membership is validated at replay time.
        let mut resources = RelationResources::new(limits);
        resources.record_term(term_nodes(source), term_depth(source))?;
        resources.record_operations(1)?;

        let rule_id = RuleId::from_rule(rule);
        let subterm = source.subterm(position).ok_or(RewriteError::InvalidPath)?;
        let substitution = match_pattern(rule.lhs(), subterm).ok_or(RewriteError::NoRule {
            rule_name: rule_id.to_string(),
        })?;
        let target_subterm = substitution.apply_matcher(rule.rhs());
        let target = source.replace_at(position, target_subterm.clone())?;

        // Erased bindings: LHS variables absent from the RHS, in
        // sorted order, keyed by schema position.
        let clause = BackwardClause::compile(rule)?;
        let mut erased_bindings = alloc::vec::Vec::new();
        for (index, name) in clause.erased_variables().iter().enumerate() {
            let variable = Variable::new(name.clone());
            let bound = substitution.get_var(&variable).cloned().ok_or(
                RewriteError::InvalidSubstitution {
                    message: alloc::string::String::from("match did not bind an erased variable"),
                },
            )?;
            erased_bindings.push((LogicVar::new(0, index as u32), bound));
        }

        let source_hash = Sha256Digest::canonical_term("amari.relation.term/v1", source);
        let target_hash = Sha256Digest::canonical_term("amari.relation.term/v1", &target);
        Ok(Self {
            target: target.clone(),
            residual: RewriteResidual {
                rule_id,
                position: position.clone(),
                erased_bindings,
                source_hash,
                target_hash,
            },
            transition: ForwardTransition {
                rule_id,
                position: position.clone(),
                from: subterm.clone(),
                to: target_subterm,
            },
        })
    }
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
