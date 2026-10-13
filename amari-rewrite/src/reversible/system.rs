// SPDX-License-Identifier: MIT OR Apache-2.0

//! Declarative bidirectional systems over checked rules, compiled
//! clauses, and residual contracts.

use alloc::string::ToString;
use alloc::vec::Vec;

use crate::analysis::InverseReport;
use crate::error::{RewriteError, RewriteResult};
use crate::inverse::{symbolic_predecessors, SymbolicPredecessor};
use crate::relation::{BackwardClause, RelationLimits, RuleId};
use crate::reversible::{ForwardTransition, ReversibleStep, RewriteResidual};
use crate::rewritable::Path;
use crate::trs::{match_pattern, Rule, Term, TermSystem};

/// A checked forward rule together with its compiled backward clause.
///
/// Runtime composite, not wire authority: serialization lives on the
/// residual/step/report types.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BidirectionalRule {
    forward: Rule,
    clause: BackwardClause,
}

impl BidirectionalRule {
    /// Compile a checked rule and its backward contract.
    pub fn compile(rule: Rule) -> RewriteResult<Self> {
        let clause = BackwardClause::compile(&rule)?;
        Ok(Self {
            forward: rule,
            clause,
        })
    }

    /// The checked forward rule.
    pub fn forward(&self) -> &Rule {
        &self.forward
    }

    /// The compiled backward contract.
    pub fn clause(&self) -> &BackwardClause {
        &self.clause
    }

    /// Canonical rule identity.
    pub fn rule_id(&self) -> RuleId {
        self.clause.rule_id()
    }
}

/// A forward step through a [`BidirectionalSystem`], with the residual
/// present exactly when requested.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub struct BidirectionalStep {
    /// The rewritten term.
    pub target: Term,
    /// The local transition performed.
    pub transition: ForwardTransition,
    /// Typed residual authority, present iff requested.
    pub residual: Option<RewriteResidual>,
}

/// A declarative bidirectional system: checked forward rules wrapped
/// with compiled backward clauses and residual contracts.
///
/// Runtime composite, not wire authority: serialization lives on the
/// residual/step/report types.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BidirectionalSystem {
    rules: Vec<BidirectionalRule>,
    forward_system: TermSystem,
}

impl BidirectionalSystem {
    /// Wrap checked forward rules with their backward contracts.
    pub fn new(rules: Vec<Rule>) -> RewriteResult<Self> {
        let compiled: Vec<BidirectionalRule> = rules
            .iter()
            .cloned()
            .map(BidirectionalRule::compile)
            .collect::<RewriteResult<_>>()?;
        Ok(Self {
            rules: compiled,
            forward_system: TermSystem::new(rules),
        })
    }

    /// The wrapped rules with their compiled clauses.
    pub fn rules(&self) -> &[BidirectionalRule] {
        &self.rules
    }

    /// The underlying forward system.
    pub fn forward_system(&self) -> &TermSystem {
        &self.forward_system
    }

    /// Concrete forward step by rule identity at `position`. When
    /// `residual` is true, full typed residual authority is emitted.
    pub fn forward_step(
        &self,
        source: &Term,
        rule_id: &RuleId,
        position: &Path,
        residual: bool,
        limits: &RelationLimits,
    ) -> RewriteResult<BidirectionalStep> {
        let rule = self
            .rules
            .iter()
            .find(|rule| rule.rule_id() == *rule_id)
            .ok_or_else(|| RewriteError::NoRule {
                rule_name: rule_id.to_string(),
            })?;
        if residual {
            let step = ReversibleStep::apply(
                &self.forward_system,
                source,
                rule.forward(),
                position,
                limits,
            )?;
            return Ok(BidirectionalStep {
                target: step.target,
                transition: step.transition,
                residual: Some(step.residual),
            });
        }
        let subterm = source.subterm(position).ok_or(RewriteError::InvalidPath)?;
        let substitution =
            match_pattern(rule.forward().lhs(), subterm).ok_or(RewriteError::NoRule {
                rule_name: rule_id.to_string(),
            })?;
        let target_subterm = substitution.apply_matcher(rule.forward().rhs());
        let target = source.replace_at(position, target_subterm.clone())?;
        Ok(BidirectionalStep {
            target,
            transition: ForwardTransition {
                rule_id: *rule_id,
                position: position.clone(),
                from: subterm.clone(),
                to: target_subterm,
            },
            residual: None,
        })
    }

    /// The symbolic predecessor relation over checked clauses.
    pub fn predecessors(
        &self,
        target: &Term,
        limits: &RelationLimits,
        scope: u32,
    ) -> RewriteResult<Vec<SymbolicPredecessor>> {
        symbolic_predecessors(&self.forward_system, target, limits, scope)
    }

    /// Exact backward replay with residual authority.
    pub fn replay(
        &self,
        residual: &RewriteResidual,
        target: &Term,
        limits: &RelationLimits,
    ) -> RewriteResult<Term> {
        residual.replay(&self.forward_system, target, limits)
    }

    /// Structural reversibility/information-loss report.
    pub fn reversibility(&self) -> InverseReport {
        crate::analysis::InverseAnalyzer::analyze(self)
    }
}
