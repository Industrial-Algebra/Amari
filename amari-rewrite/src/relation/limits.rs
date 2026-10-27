// SPDX-License-Identifier: MIT OR Apache-2.0

//! Relation limits and resource accounting.
//!
//! Fixed ceilings are associated constants and cannot be raised by
//! callers; [`RelationLimits::new`] only accepts tightened values.
//! Limits are validated before allocation, recursion, or search, and
//! [`RelationResources`] enforces them at runtime.

use crate::error::{RewriteError, RewriteResult};

/// Caller-tightenable limits for constrained relation evaluation.
///
/// The fields are private and every constructor validates, so a
/// `RelationLimits` in hand is a validated value — including one that came
/// off the wire, because `serde::Deserialize` routes through [`Self::new`]
/// (PR #353 round 4 P2b).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize))]
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

    /// Clamp every ceiling to the hard limits.
    ///
    /// Defence in depth for the use sites: even a limits value built
    /// outside the validating constructors — a future struct literal, a
    /// decoder that forgot to validate — cannot raise a hard ceiling once
    /// the encoder or the resource budget has it (PR #353 round 4 P2b).
    pub(crate) fn clamped(&self) -> Self {
        Self {
            max_term_nodes: self.max_term_nodes.clamp(1, Self::MAX_TERM_NODES),
            max_term_depth: self.max_term_depth.clamp(1, Self::MAX_TERM_DEPTH),
            max_constraints: self.max_constraints.clamp(1, Self::MAX_CONSTRAINTS),
            max_operations: self.max_operations.clamp(1, Self::MAX_OPERATIONS),
        }
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
#[cfg_attr(feature = "serialize", derive(serde::Serialize))]
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
    ///
    /// The ceilings are clamped to the hard limits, so a budget can never
    /// enforce more authority than the design grants, whatever the
    /// provenance of `limits` (PR #353 round 4 P2b defence in depth).
    pub fn new(limits: &RelationLimits) -> Self {
        let limits = limits.clamped();
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

/// The wire form of [`RelationLimits`]: validation happens after decoding,
/// so the decoded values never reach a constructor-free struct literal.
#[cfg(feature = "serialize")]
#[derive(serde::Deserialize)]
struct RelationLimitsWire {
    max_term_nodes: usize,
    max_term_depth: usize,
    max_constraints: usize,
    max_operations: usize,
}

#[cfg(feature = "serialize")]
impl<'de> serde::Deserialize<'de> for RelationLimits {
    /// Decode and validate: the hard ceilings cannot be raised on the wire.
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = RelationLimitsWire::deserialize(deserializer)?;
        Self::new(
            wire.max_term_nodes,
            wire.max_term_depth,
            wire.max_constraints,
            wire.max_operations,
        )
        .map_err(serde::de::Error::custom)
    }
}

/// The wire form of [`RelationResources`].
#[cfg(feature = "serialize")]
#[derive(serde::Deserialize)]
struct RelationResourcesWire {
    max_term_nodes: usize,
    max_term_depth: usize,
    max_constraints: usize,
    max_operations: usize,
    constraints: usize,
    operations: usize,
}

#[cfg(feature = "serialize")]
impl<'de> serde::Deserialize<'de> for RelationResources {
    /// Decode and validate: a budget cannot arrive with raised ceilings or
    /// with usage beyond its own maximum.
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = RelationResourcesWire::deserialize(deserializer)?;
        let limits = RelationLimits::new(
            wire.max_term_nodes,
            wire.max_term_depth,
            wire.max_constraints,
            wire.max_operations,
        )
        .map_err(serde::de::Error::custom)?;
        if wire.constraints > limits.max_constraints() {
            return Err(serde::de::Error::custom(format!(
                "constraints accounted ({}) exceed the decoded limit ({})",
                wire.constraints,
                limits.max_constraints()
            )));
        }
        if wire.operations > limits.max_operations() {
            return Err(serde::de::Error::custom(format!(
                "operations accounted ({}) exceed the decoded limit ({})",
                wire.operations,
                limits.max_operations()
            )));
        }
        Ok(Self {
            max_term_nodes: limits.max_term_nodes(),
            max_term_depth: limits.max_term_depth(),
            max_constraints: limits.max_constraints(),
            max_operations: limits.max_operations(),
            constraints: wire.constraints,
            operations: wire.operations,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A struct literal bypasses [`RelationLimits::new`]; the use sites must
    /// not inherit the raised ceilings (PR #353 round 4 P2b).
    fn raised() -> RelationLimits {
        RelationLimits {
            max_term_nodes: 20_000,
            max_term_depth: 10_001,
            max_constraints: RelationLimits::MAX_CONSTRAINTS,
            max_operations: RelationLimits::MAX_OPERATIONS,
        }
    }

    #[test]
    fn clamping_cannot_raise_a_hard_ceiling() {
        let clamped = raised().clamped();
        assert_eq!(clamped.max_term_nodes(), RelationLimits::MAX_TERM_NODES);
        assert_eq!(clamped.max_term_depth(), RelationLimits::MAX_TERM_DEPTH);
    }

    #[test]
    fn clamping_floors_a_disabled_ceiling() {
        let zeroed = RelationLimits {
            max_term_nodes: 0,
            max_term_depth: 0,
            max_constraints: 0,
            max_operations: 0,
        };
        let clamped = zeroed.clamped();
        assert_eq!(clamped.max_term_nodes(), 1);
        assert_eq!(clamped.max_term_depth(), 1);
        assert_eq!(clamped.max_constraints(), 1);
        assert_eq!(clamped.max_operations(), 1);
    }

    #[test]
    fn a_resource_budget_clamps_the_limits_it_is_opened_with() {
        let mut budget = RelationResources::new(&raised());
        assert!(budget.record_term(1, 65).is_err());
        assert!(budget
            .record_term(RelationLimits::MAX_TERM_NODES + 1, 1)
            .is_err());
        assert!(budget
            .record_term(RelationLimits::MAX_TERM_NODES, 1)
            .is_ok());
    }

    #[cfg(feature = "neural")]
    #[test]
    fn an_encoder_clamps_the_limits_it_is_built_with() {
        let encoder = crate::neural::FeatureEncoder::new(raised());
        assert_eq!(
            encoder.limits().max_term_depth(),
            RelationLimits::MAX_TERM_DEPTH
        );
        assert_eq!(
            encoder.limits().max_term_nodes(),
            RelationLimits::MAX_TERM_NODES
        );
    }
}
