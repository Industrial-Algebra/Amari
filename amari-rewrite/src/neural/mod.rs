//! Experimental neural and differentiable rewrite traits.
//!
//! This module is intentionally trait-only in 0.23.0. It keeps the rewrite
//! architecture open for learned rules and neural inverse rewriting without
//! choosing a tensor framework or adding heavy default dependencies.

/// A differentiable or learned rewrite rule over a state representation.
pub trait DifferentiableRule<State> {
    /// Rule parameters, if any.
    type Parameters;
    /// Gradient representation, if any.
    type Gradient;
    /// Rule evaluation error.
    type Error;

    /// Apply the rule forward.
    fn forward(&self, state: &State) -> Result<State, Self::Error>;

    /// Compute a scalar loss between a prediction and a target.
    fn loss(&self, predicted: &State, target: &State) -> Result<f64, Self::Error>;
}
