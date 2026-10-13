// SPDX-License-Identifier: MIT OR Apache-2.0

//! Backward clauses compiled from checked rules.
//!
//! A [`BackwardClause`] wraps a checked [`Rule`] with its stable
//! identity digest and precomputed erased variables (LHS variables
//! absent from the RHS — the information a forward step destroys).
//! Freshening allocates from a deterministic query-scoped
//! [`LogicVarNamespace`] so backward applications never capture target
//! or rule variables.

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use core::fmt;

use crate::error::{RewriteError, RewriteResult};
use crate::relation::digest::encode_term;
use crate::relation::{LogicVar, LogicVarNamespace, Sha256Digest};
use crate::trs::{Rule, Substitution, Term};

/// Stable rule identity: the canonical digest of the checked rule's
/// LHS/RHS pair under the rule frame.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub struct RuleId(Sha256Digest);

impl RuleId {
    /// Compute the identity of a checked rule.
    pub fn from_rule(rule: &Rule) -> Self {
        let mut variables = Vec::new();
        let mut encoding = Vec::new();
        encode_term(rule.lhs(), &mut variables, &mut encoding);
        encode_term(rule.rhs(), &mut variables, &mut encoding);
        Self(Sha256Digest::framed("amari.relation.rule/v1", &encoding))
    }

    /// The underlying digest.
    pub fn digest(&self) -> Sha256Digest {
        self.0
    }
}

impl fmt::Display for RuleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Debug for RuleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RuleId({})", self.0)
    }
}

/// A rule compiled for backward application.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub struct BackwardClause {
    rule_id: RuleId,
    lhs: Term,
    rhs: Term,
    /// LHS variables absent from the RHS (erased forward; existential
    /// backward), in sorted order.
    erased: Vec<String>,
}

impl BackwardClause {
    /// Compile a checked rule. Defensively re-verifies the checked
    /// invariant (RHS variables ⊆ LHS variables): a clause must never
    /// exist for a rule that invents information backward.
    pub fn compile(rule: &Rule) -> RewriteResult<Self> {
        let lhs_vars: Vec<String> = rule
            .lhs()
            .variables()
            .iter()
            .map(ToString::to_string)
            .collect();
        for variable in rule.rhs().variables() {
            let name = variable.to_string();
            if !lhs_vars.contains(&name) {
                return Err(RewriteError::InvalidRule {
                    message: String::from(
                        "backward clause requires RHS variables to occur \
                         in the LHS",
                    ),
                });
            }
        }
        let mut erased: Vec<String> = {
            let rhs_vars: Vec<String> = rule
                .rhs()
                .variables()
                .iter()
                .map(ToString::to_string)
                .collect();
            lhs_vars
                .iter()
                .filter(|name| !rhs_vars.contains(*name))
                .cloned()
                .collect()
        };
        erased.sort();
        erased.dedup();
        Ok(Self {
            rule_id: RuleId::from_rule(rule),
            lhs: rule.lhs().clone(),
            rhs: rule.rhs().clone(),
            erased,
        })
    }

    /// Stable rule identity.
    pub fn rule_id(&self) -> RuleId {
        self.rule_id
    }

    /// Variables erased by the forward direction (existential
    /// backward), sorted.
    pub fn erased_variables(&self) -> &[String] {
        &self.erased
    }

    /// Freshen the clause under the namespace: rule variables become
    /// logic variables named by their `LogicVar` display form.
    pub(crate) fn freshen(&self, namespace: &mut LogicVarNamespace) -> FreshenedClause {
        let mut renaming = Substitution::new();
        let mut erased_vars = Vec::new();
        let mut variables: Vec<String> = self
            .lhs
            .variables()
            .iter()
            .map(ToString::to_string)
            .collect();
        variables.sort();
        variables.dedup();
        for name in &variables {
            let logic = namespace.fresh();
            renaming.insert(name.clone(), Term::var(logic.to_string()));
            if self.erased.contains(name) {
                erased_vars.push(logic);
            }
        }
        FreshenedClause {
            lhs: renaming.apply_matcher(&self.lhs),
            rhs: renaming.apply_matcher(&self.rhs),
            erased: erased_vars,
        }
    }
}

/// A clause with rule variables renamed into a query scope.
pub(crate) struct FreshenedClause {
    pub lhs: Term,
    pub rhs: Term,
    /// Logic variables for the erased (existential) variables.
    pub erased: Vec<LogicVar>,
}
