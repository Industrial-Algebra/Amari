// SPDX-License-Identifier: MIT OR Apache-2.0

//! Tree automaton limits.
//!
//! Fixed ceilings are associated constants and cannot be raised by
//! callers; [`TreeAutomatonLimits::new`] only accepts tightened
//! values. Limits are checked before allocation at construction
//! time.

use crate::error::{RewriteError, RewriteResult};

/// Caller-tightenable limits for tree automata.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub struct TreeAutomatonLimits {
    max_states: usize,
    max_transitions: usize,
    max_rank: usize,
}

impl TreeAutomatonLimits {
    /// Hard ceiling for automaton states (design resource authority).
    pub const MAX_STATES: usize = 4_096;
    /// Hard ceiling for automaton transitions.
    pub const MAX_TRANSITIONS: usize = 65_536;
    /// Hard ceiling for symbol rank.
    pub const MAX_RANK: usize = 16;

    /// Construct tightened limits. Zero or above-ceiling values are
    /// rejected: limits exist to bound work, not to disable or
    /// expand the authority.
    pub fn new(max_states: usize, max_transitions: usize, max_rank: usize) -> RewriteResult<Self> {
        for (resource, value, ceiling) in [
            ("tree automaton states", max_states, Self::MAX_STATES),
            (
                "tree automaton transitions",
                max_transitions,
                Self::MAX_TRANSITIONS,
            ),
            ("tree automaton rank", max_rank, Self::MAX_RANK),
        ] {
            if value == 0 || value > ceiling {
                return Err(RewriteError::InvalidLimit {
                    resource,
                    value,
                    ceiling,
                });
            }
        }
        Ok(Self {
            max_states,
            max_transitions,
            max_rank,
        })
    }

    /// The state limit.
    pub fn max_states(&self) -> usize {
        self.max_states
    }

    /// The transition limit.
    pub fn max_transitions(&self) -> usize {
        self.max_transitions
    }

    /// The rank limit.
    pub fn max_rank(&self) -> usize {
        self.max_rank
    }
}

impl Default for TreeAutomatonLimits {
    /// The full authority: every ceiling at its fixed maximum.
    fn default() -> Self {
        Self {
            max_states: Self::MAX_STATES,
            max_transitions: Self::MAX_TRANSITIONS,
            max_rank: Self::MAX_RANK,
        }
    }
}
