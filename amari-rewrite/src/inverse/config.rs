// SPDX-License-Identifier: MIT OR Apache-2.0

//! Inverse search configuration with fixed ceilings.
//!
//! Caller-supplied values below the ceilings tighten the search;
//! zero or above-ceiling values are typed errors. Ceilings are
//! associated constants and cannot be raised by callers (design
//! resource-authority table: 65,536 states, 262,144 transitions,
//! depth 64, 64 MiB retained evidence).

use crate::error::{RewriteError, RewriteResult};

/// Bounded configuration for backward/bidirectional inverse search.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(
    feature = "serialize",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields)
)]
pub struct InverseSearchConfig {
    max_depth: u64,
    max_states: u64,
    max_transitions: u64,
    max_term_nodes: u64,
    max_term_depth: u64,
    max_constraints: u64,
    max_groundings: u64,
    max_operations: u64,
    max_frontier_bytes: u64,
    max_trace_bytes: u64,
}

impl InverseSearchConfig {
    /// Hard ceiling on search depth.
    pub const MAX_DEPTH: u64 = 64;
    /// Hard ceiling on retained states.
    pub const MAX_STATES: u64 = 65_536;
    /// Hard ceiling on explored transitions.
    pub const MAX_TRANSITIONS: u64 = 262_144;
    /// Hard ceiling on term nodes.
    pub const MAX_TERM_NODES: u64 = 4_096;
    /// Hard ceiling on term depth.
    pub const MAX_TERM_DEPTH: u64 = 64;
    /// Hard ceiling on constraints per state.
    pub const MAX_CONSTRAINTS: u64 = 4_096;
    /// Hard ceiling on grounding assignments.
    pub const MAX_GROUNDINGS: u64 = 65_536;
    /// Hard ceiling on abstract operations.
    pub const MAX_OPERATIONS: u64 = 1_000_000;
    /// Hard ceiling on retained frontier evidence bytes (64 MiB).
    pub const MAX_RETAINED_BYTES: u64 = 64 * 1024 * 1024;
    /// Hard ceiling on trace bytes (64 MiB).
    pub const MAX_TRACE_BYTES: u64 = 64 * 1024 * 1024;

    /// Validate a search configuration. Zero or above-ceiling values
    /// are [`RewriteError::InvalidLimit`] errors.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        max_depth: u64,
        max_states: u64,
        max_transitions: u64,
        max_term_nodes: u64,
        max_term_depth: u64,
        max_constraints: u64,
        max_groundings: u64,
        max_operations: u64,
        max_frontier_bytes: u64,
        max_trace_bytes: u64,
    ) -> RewriteResult<Self> {
        let config = Self {
            max_depth,
            max_states,
            max_transitions,
            max_term_nodes,
            max_term_depth,
            max_constraints,
            max_groundings,
            max_operations,
            max_frontier_bytes,
            max_trace_bytes,
        };
        config.check("search depth", config.max_depth, Self::MAX_DEPTH)?;
        config.check("search states", config.max_states, Self::MAX_STATES)?;
        config.check(
            "search transitions",
            config.max_transitions,
            Self::MAX_TRANSITIONS,
        )?;
        config.check(
            "search term nodes",
            config.max_term_nodes,
            Self::MAX_TERM_NODES,
        )?;
        config.check(
            "search term depth",
            config.max_term_depth,
            Self::MAX_TERM_DEPTH,
        )?;
        config.check(
            "search constraints",
            config.max_constraints,
            Self::MAX_CONSTRAINTS,
        )?;
        config.check(
            "search groundings",
            config.max_groundings,
            Self::MAX_GROUNDINGS,
        )?;
        config.check(
            "search operations",
            config.max_operations,
            Self::MAX_OPERATIONS,
        )?;
        config.check(
            "search frontier bytes",
            config.max_frontier_bytes,
            Self::MAX_RETAINED_BYTES,
        )?;
        config.check(
            "search trace bytes",
            config.max_trace_bytes,
            Self::MAX_TRACE_BYTES,
        )?;
        Ok(config)
    }

    fn check(&self, resource: &'static str, value: u64, ceiling: u64) -> RewriteResult<()> {
        if value == 0 || value > ceiling {
            return Err(RewriteError::InvalidLimit {
                resource,
                value: value as usize,
                ceiling: ceiling as usize,
            });
        }
        Ok(())
    }

    /// Search depth cap.
    pub fn max_depth(&self) -> u64 {
        self.max_depth
    }

    /// Retained-state cap.
    pub fn max_states(&self) -> u64 {
        self.max_states
    }

    /// Explored-transition cap.
    pub fn max_transitions(&self) -> u64 {
        self.max_transitions
    }

    /// Term-node cap.
    pub fn max_term_nodes(&self) -> u64 {
        self.max_term_nodes
    }

    /// Term-depth cap.
    pub fn max_term_depth(&self) -> u64 {
        self.max_term_depth
    }

    /// Per-state constraint cap.
    pub fn max_constraints(&self) -> u64 {
        self.max_constraints
    }

    /// Grounding-assignment cap.
    pub fn max_groundings(&self) -> u64 {
        self.max_groundings
    }

    /// Operation cap.
    pub fn max_operations(&self) -> u64 {
        self.max_operations
    }

    /// Retained frontier byte cap.
    pub fn max_frontier_bytes(&self) -> u64 {
        self.max_frontier_bytes
    }

    /// Trace byte cap.
    pub fn max_trace_bytes(&self) -> u64 {
        self.max_trace_bytes
    }
}

impl Default for InverseSearchConfig {
    /// Defaults sit at the fixed ceilings.
    fn default() -> Self {
        Self {
            max_depth: Self::MAX_DEPTH,
            max_states: Self::MAX_STATES,
            max_transitions: Self::MAX_TRANSITIONS,
            max_term_nodes: Self::MAX_TERM_NODES,
            max_term_depth: Self::MAX_TERM_DEPTH,
            max_constraints: Self::MAX_CONSTRAINTS,
            max_groundings: Self::MAX_GROUNDINGS,
            max_operations: Self::MAX_OPERATIONS,
            max_frontier_bytes: Self::MAX_RETAINED_BYTES,
            max_trace_bytes: Self::MAX_TRACE_BYTES,
        }
    }
}
