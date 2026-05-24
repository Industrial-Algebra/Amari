//! Term rewrite rules.

use alloc::{collections::BTreeSet, string::String};

use crate::{RewriteError, RewriteResult};

use super::{match_pattern, Substitution, Term, Variable};

/// A first-order term rewrite rule `lhs -> rhs`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rule {
    lhs: Term,
    rhs: Term,
}

impl Rule {
    /// Create a checked rule.
    ///
    /// All variables in the RHS must occur in the LHS.
    pub fn new(lhs: Term, rhs: Term) -> RewriteResult<Self> {
        let lhs_vars: BTreeSet<Variable> = lhs.variables().into_iter().collect();
        let rhs_vars = rhs.variables();
        if let Some(missing) = rhs_vars.iter().find(|var| !lhs_vars.contains(*var)) {
            return Err(RewriteError::InvalidRule {
                message: format_alloc("rhs variable ", missing.as_str(), " does not occur in lhs"),
            });
        }
        Ok(Self { lhs, rhs })
    }

    /// Create a rule without RHS variable validation.
    pub fn new_unchecked(lhs: Term, rhs: Term) -> Self {
        Self { lhs, rhs }
    }

    /// Borrow the left-hand side pattern.
    pub fn lhs(&self) -> &Term {
        &self.lhs
    }

    /// Borrow the right-hand side template.
    pub fn rhs(&self) -> &Term {
        &self.rhs
    }

    /// Match the LHS against `term`.
    pub fn matches(&self, term: &Term) -> Option<Substitution> {
        match_pattern(&self.lhs, term)
    }

    /// Apply this rule at the root of `term`.
    pub fn apply_root(&self, term: &Term) -> Option<Term> {
        let subst = self.matches(term)?;
        Some(subst.apply(&self.rhs))
    }
}

fn format_alloc(prefix: &str, value: &str, suffix: &str) -> String {
    let mut out = String::new();
    out.push_str(prefix);
    out.push_str(value);
    out.push_str(suffix);
    out
}
