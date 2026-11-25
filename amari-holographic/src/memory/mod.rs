//! Additive associative memory over checked binding algebras.
//!
//! Storage adds the ordered binding `key.bind(value)` to a raw trace. Retrieval
//! left-multiplies the trace by the checked key inverse; it neither normalizes
//! the result nor performs codebook cleanup. Duplicate records add twice.
//!
//! With tracking enabled, exact membership and nearest-key similarity are
//! separate queries. An untracked memory cannot answer membership. Load-based
//! capacity heuristics are not evidence that any particular key was stored.
//!
//! Use [`Resonator::cleanup_nearest`] to select a codeword or
//! [`Resonator::cleanup_weighted`] for iterative mixing. Both distinguish a
//! candidate from a reconstruction that meets the observation residual.

pub(crate) mod domain;
pub use domain::MemoryElement;
mod error;
mod holographic_memory;
mod resonator;

pub use crate::algebra::BindingAlgebra as Bindable;
pub use error::{HolographicError, HolographicResult};
pub use holographic_memory::{
    CapacityInfo, HolographicMemory, MembershipEvidence, NearestKey, RetrievalResult,
};
pub use resonator::{
    CleanupResult, FactorizationResult, Resonator, ResonatorConfig, WeightedResult, WeightedStatus,
};
