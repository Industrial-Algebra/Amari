// SPDX-License-Identifier: MIT OR Apache-2.0

//! Bounded completion trace.
//!
//! The trace records completion EVENTS as compact descriptors — rule
//! indices and overlap provenance, never whole terms — so a bounded
//! run cannot accumulate unbounded evidence. The counters are
//! consistent by construction: `pairs_processed` is the number of
//! [`TraceStep::Pair`] steps and `rules_added` the number of
//! [`TraceStep::Oriented`] steps.

use alloc::vec::Vec;

use crate::Path;

/// One compact completion event.
///
/// Every variant stores only indices and a path: a trace is bounded
/// by the number of events, never by the size of the terms involved.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TraceStep {
    /// A distinct critical-pair equation was admitted for processing.
    Pair {
        /// Index of the overlapped (outer) rule.
        left_rule: usize,
        /// Index of the rule unified into the position.
        right_rule: usize,
        /// Overlap position inside the outer rule's left-hand side.
        path: Path,
    },
    /// A generated equation was oriented and added at this index.
    Oriented {
        /// Index of the new rule in the current system.
        rule_index: usize,
    },
    /// An existing rule's right-hand side was reduced by the system.
    Simplified {
        /// Index of the rule whose right-hand side changed.
        rule_index: usize,
    },
}

/// Bounded, metered record of a completion run.
///
/// `rules_added` excludes the oriented input rules; it counts only
/// the rules completion discovered from critical pairs.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CompletionTrace {
    /// Rules generated and added by completion.
    pub rules_added: usize,
    /// Distinct critical-pair equations processed.
    pub pairs_processed: usize,
    /// Compact event list in deterministic order.
    pub steps: Vec<TraceStep>,
}
