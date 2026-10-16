// SPDX-License-Identifier: MIT OR Apache-2.0

//! Configuration for bounded Knuth–Bendix completion.

use crate::analysis::Precedence;

/// Orientation order used by completion.
///
/// Completion never invents a precedence: the caller supplies the
/// validated [`Precedence`] (Task 32) that orients every rule. The
/// same order drives the input orientation, the critical-pair
/// orientation, and the final termination postcheck — a single
/// authority keeps the intermediate and final systems honest.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompletionConfig {
    /// The LPO precedence used to orient every generated equation.
    pub precedence: Precedence,
}

impl CompletionConfig {
    /// Build a configuration from a validated precedence.
    pub fn new(precedence: Precedence) -> Self {
        Self { precedence }
    }
}
