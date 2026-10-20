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
use crate::relation::digest::{encode_term, encode_term_metered};
use crate::relation::{LogicVar, LogicVarNamespace, RelationResources, Sha256Digest};
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

    /// Metered identity of a checked rule.
    ///
    /// Charges the shared budget for the alpha-canonical encoding
    /// (per node, per copied name, per renumbered variable entry) and
    /// for the payload bytes the hash reads, all before the work runs
    /// (PR #335 round 1 P2a).
    pub(crate) fn from_rule_metered(
        rule: &Rule,
        resources: &mut RelationResources,
    ) -> RewriteResult<Self> {
        let mut variables = Vec::new();
        let mut encoding = Vec::new();
        encode_term_metered(rule.lhs(), &mut variables, &mut encoding, resources)?;
        encode_term_metered(rule.rhs(), &mut variables, &mut encoding, resources)?;
        // The digest reads every payload byte and the frame: charge
        // them BEFORE the hash is computed.
        resources.record_operations(encoding.len().saturating_add(1))?;
        Ok(Self(Sha256Digest::framed(
            "amari.relation.rule/v1",
            &encoding,
        )))
    }

    /// The underlying digest.
    pub fn digest(&self) -> Sha256Digest {
        self.0
    }
}

/// Sorted, deduplicated variable names of a term.
///
/// Charges one unit per visited node, one per copied name, and one per
/// sort and dedup entry before the work runs. Returns the visited node
/// count so callers can charge the clone they take from the same
/// term.
fn metered_variable_names(
    term: &Term,
    resources: &mut RelationResources,
) -> RewriteResult<(usize, Vec<String>)> {
    let mut names: Vec<String> = Vec::new();
    let mut nodes = 0usize;
    let mut stack = Vec::from([term]);
    while let Some(node) = stack.pop() {
        resources.record_operations(1)?;
        nodes = nodes.saturating_add(1);
        match node {
            Term::Var(variable) => {
                resources.record_operations(1)?;
                names.push(variable.to_string());
            }
            Term::Sym(_, args) => stack.extend(args.iter()),
        }
    }
    resources.record_operations(names.len())?;
    names.sort();
    resources.record_operations(names.len())?;
    names.dedup();
    Ok((nodes, names))
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
    ///
    /// Unmetered: delegates to the metered compilation under an
    /// unbounded budget, which can never be exhausted. Metered callers
    /// (inverse synthesis) charge compilation to their own budget
    /// instead.
    pub fn compile(rule: &Rule) -> RewriteResult<Self> {
        let mut resources = RelationResources::unbounded();
        Self::compile_metered(rule, &mut resources)
    }

    /// Compile a checked rule against a caller-supplied budget.
    ///
    /// Every unit is charged before the work it meters: per visited
    /// node of both sides, per copied variable name, per sort/dedup
    /// entry, per visited lhs entry of the RHS-subset and erased-set
    /// scans, per encoded node and per renumbered variable entry of
    /// the rule identity digest, and per node of the two-sided clone.
    /// Those scans are quadratic in the distinct-variable count, so a
    /// single per-term charge would hide thousands of comparisons
    /// (PR #335 round 1 P2a).
    pub(crate) fn compile_metered(
        rule: &Rule,
        resources: &mut RelationResources,
    ) -> RewriteResult<Self> {
        let (lhs_nodes, lhs_vars) = metered_variable_names(rule.lhs(), resources)?;
        let (rhs_nodes, rhs_vars) = metered_variable_names(rule.rhs(), resources)?;

        // Defensive invariant: every RHS variable occurs in the LHS.
        // The membership scan charges every visited entry BEFORE the
        // comparison runs.
        for variable in &rhs_vars {
            let mut found = false;
            for candidate in &lhs_vars {
                resources.record_operations(1)?;
                if candidate == variable {
                    found = true;
                    break;
                }
            }
            if !found {
                return Err(RewriteError::InvalidRule {
                    message: String::from(
                        "backward clause requires RHS variables to occur \
                         in the LHS",
                    ),
                });
            }
        }

        // Erased set: LHS variables absent from the RHS, sorted.
        let mut erased: Vec<String> = Vec::new();
        resources.record_operations(lhs_vars.len())?;
        for candidate in &lhs_vars {
            let mut present = false;
            for variable in &rhs_vars {
                resources.record_operations(1)?;
                if variable == candidate {
                    present = true;
                    break;
                }
            }
            if !present {
                resources.record_operations(1)?;
                erased.push(candidate.clone());
            }
        }

        let rule_id = RuleId::from_rule_metered(rule, resources)?;

        // Clone both sides: charge per node BEFORE the clone runs.
        resources.record_operations(lhs_nodes.saturating_add(rhs_nodes))?;
        Ok(Self {
            rule_id,
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
    pub(crate) fn freshen(
        &self,
        namespace: &mut LogicVarNamespace,
    ) -> RewriteResult<FreshenedClause> {
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
            let logic = namespace
                .fresh()
                .ok_or(RewriteError::RelationLimitExceeded {
                    resource: "logic variable namespace",
                    limit: u32::MAX as usize,
                })?;
            renaming.insert(name.clone(), Term::var(logic.to_string()));
            if self.erased.contains(name) {
                erased_vars.push(logic);
            }
        }
        Ok(FreshenedClause {
            lhs: renaming.apply_matcher(&self.lhs),
            rhs: renaming.apply_matcher(&self.rhs),
            erased: erased_vars,
        })
    }
}

/// A clause with rule variables renamed into a query scope.
pub(crate) struct FreshenedClause {
    pub lhs: Term,
    pub rhs: Term,
    /// Logic variables for the erased (existential) variables.
    pub erased: Vec<LogicVar>,
}
