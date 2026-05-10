//! Experimental `amari-network` bridge for rewrite search guidance.
//!
//! Rewrite systems naturally form directed graphs: terms are nodes and rewrite
//! steps are edges. In 0.23.0 this module only exposes lightweight summaries and
//! keeps richer `GeometricNetwork` adapters as future experimental work.

/// Summary of a rewrite trace as a graph path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RewriteGraphSummary {
    /// Number of terms in the trace.
    pub nodes: usize,
    /// Number of rewrite transitions in the trace.
    pub edges: usize,
}

impl RewriteGraphSummary {
    /// Summarize a linear rewrite trace.
    pub fn from_trace<T>(trace: &[T]) -> Self {
        Self {
            nodes: trace.len(),
            edges: trace.len().saturating_sub(1),
        }
    }
}

/// Marker proving this module was compiled with the optional `amari-network`
/// dependency available.
pub fn network_bridge_enabled() -> bool {
    let _ = core::mem::size_of::<Option<amari_network::NodeMetadata>>();
    true
}
