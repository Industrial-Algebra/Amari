// SPDX-License-Identifier: MIT OR Apache-2.0

//! Relation limits and resource accounting.
//!
//! Fixed ceilings are associated constants and cannot be raised by
//! callers; [`RelationLimits::new`] only accepts tightened values.
//! Limits are validated before allocation, recursion, or search, and
//! [`RelationResources`] enforces them at runtime.

use crate::error::{RewriteError, RewriteResult};

/// Caller-tightenable limits for constrained relation evaluation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub struct RelationLimits {
    max_term_nodes: usize,
    max_term_depth: usize,
    max_constraints: usize,
    max_operations: usize,
}

impl RelationLimits {
    /// Hard ceiling for nodes per term (design resource authority).
    pub const MAX_TERM_NODES: usize = 4_096;
    /// Hard ceiling for term depth.
    pub const MAX_TERM_DEPTH: usize = 64;
    /// Hard ceiling for constraints per constraint set.
    pub const MAX_CONSTRAINTS: usize = 4_096;
    /// Hard ceiling for accounted operations per query.
    pub const MAX_OPERATIONS: usize = 1_000_000;

    /// Construct tightened limits. Zero or above-ceiling values are
    /// rejected: limits exist to bound work, not to disable or expand
    /// the authority.
    pub fn new(
        max_term_nodes: usize,
        max_term_depth: usize,
        max_constraints: usize,
        max_operations: usize,
    ) -> RewriteResult<Self> {
        let candidate = Self {
            max_term_nodes,
            max_term_depth,
            max_constraints,
            max_operations,
        };
        candidate.check("term nodes", max_term_nodes, Self::MAX_TERM_NODES)?;
        candidate.check("term depth", max_term_depth, Self::MAX_TERM_DEPTH)?;
        candidate.check("constraints", max_constraints, Self::MAX_CONSTRAINTS)?;
        candidate.check("operations", max_operations, Self::MAX_OPERATIONS)?;
        Ok(candidate)
    }

    fn check(&self, resource: &'static str, value: usize, ceiling: usize) -> RewriteResult<()> {
        let _ = self;
        if value == 0 || value > ceiling {
            return Err(RewriteError::InvalidLimit {
                resource,
                value,
                ceiling,
            });
        }
        Ok(())
    }

    /// Maximum nodes per term.
    pub fn max_term_nodes(&self) -> usize {
        self.max_term_nodes
    }

    /// Maximum term depth.
    pub fn max_term_depth(&self) -> usize {
        self.max_term_depth
    }

    /// Maximum constraints per constraint set.
    pub fn max_constraints(&self) -> usize {
        self.max_constraints
    }

    /// Maximum accounted operations per query.
    pub fn max_operations(&self) -> usize {
        self.max_operations
    }
}

impl Default for RelationLimits {
    /// The default profile is the fixed ceiling profile.
    fn default() -> Self {
        Self {
            max_term_nodes: Self::MAX_TERM_NODES,
            max_term_depth: Self::MAX_TERM_DEPTH,
            max_constraints: Self::MAX_CONSTRAINTS,
            max_operations: Self::MAX_OPERATIONS,
        }
    }
}

/// Running resource budget for one relation query.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub struct RelationResources {
    max_term_nodes: usize,
    max_term_depth: usize,
    max_constraints: usize,
    max_operations: usize,
    constraints: usize,
    operations: usize,
}

impl RelationResources {
    /// Open a budget under the given limits.
    pub fn new(limits: &RelationLimits) -> Self {
        Self {
            max_term_nodes: limits.max_term_nodes,
            max_term_depth: limits.max_term_depth,
            max_constraints: limits.max_constraints,
            max_operations: limits.max_operations,
            constraints: 0,
            operations: 0,
        }
    }

    /// Account one term of `nodes` nodes and `depth` depth. Per-term
    /// bounds are per value, not cumulative.
    pub fn record_term(&mut self, nodes: usize, depth: usize) -> RewriteResult<()> {
        if nodes > self.max_term_nodes {
            return Err(RewriteError::RelationLimitExceeded {
                resource: "term nodes",
                limit: self.max_term_nodes,
            });
        }
        if depth > self.max_term_depth {
            return Err(RewriteError::RelationLimitExceeded {
                resource: "term depth",
                limit: self.max_term_depth,
            });
        }
        Ok(())
    }

    /// Account additional constraints (cumulative).
    pub fn record_constraints(&mut self, additional: usize) -> RewriteResult<()> {
        let next = self.constraints.saturating_add(additional);
        if next > self.max_constraints {
            return Err(RewriteError::RelationLimitExceeded {
                resource: "constraints",
                limit: self.max_constraints,
            });
        }
        self.constraints = next;
        Ok(())
    }

    /// Account additional operations (cumulative).
    pub fn record_operations(&mut self, additional: usize) -> RewriteResult<()> {
        let next = self.operations.saturating_add(additional);
        if next > self.max_operations {
            return Err(RewriteError::RelationLimitExceeded {
                resource: "operations",
                limit: self.max_operations,
            });
        }
        self.operations = next;
        Ok(())
    }

    /// An unbounded budget for non-fallible internal callers: counts
    /// still accumulate, but nothing is ever enforced. Metered
    /// (budgeted) entry points take a caller-supplied budget instead.
    pub(crate) fn unbounded() -> Self {
        Self {
            max_term_nodes: usize::MAX,
            max_term_depth: usize::MAX,
            max_constraints: usize::MAX,
            max_operations: usize::MAX,
            constraints: 0,
            operations: 0,
        }
    }

    /// Constraints accounted so far.
    pub fn constraints(&self) -> usize {
        self.constraints
    }

    /// Operations accounted so far.
    pub fn operations(&self) -> usize {
        self.operations
    }
}
