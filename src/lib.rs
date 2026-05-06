//! Amari: Advanced Mathematical Algebra for Robust Intelligence
//!
//! A comprehensive mathematical computing library combining:
//! - Geometric algebra (Clifford algebras)
//! - Tropical algebra (max-plus semiring plus ordinal-weighted optimization carriers)
//! - Dual number automatic differentiation (forward-mode, including fixed-size gradient loops)
//! - Geometric network analysis
//! - Information geometry
//! - Fusion systems for neural network optimization
//! - Measure theory and Lebesgue integration (opt-in via `measure` feature)
//! - Differential calculus on geometric algebra (opt-in via `calculus` feature)
//! - Holographic memory / Vector Symbolic Architectures (opt-in via `holographic` feature)
//! - Probability theory on geometric algebra (opt-in via `probabilistic` feature)
//! - Probabilistic contracts and verification (opt-in via `flynn` feature)
//! - Deterministic physics for networked applications (opt-in via `deterministic` feature)
//! - Functional analysis on multivector spaces (opt-in via `functional` feature)
//! - Algebraic topology: homology, persistent homology, Morse theory (opt-in via `topology` feature)
//! - Dynamical systems: ODE solvers, stability, bifurcations, chaos (opt-in via `dynamics` feature)
//!
//! Use `features = ["full"]` to enable all optional crates.

pub use amari_automata as automata;
pub use amari_core as core;
pub use amari_dual as dual;
pub use amari_enumerative as enumerative;
pub use amari_fusion as fusion;
pub use amari_info_geom as info_geom;
pub use amari_network as network;
pub use amari_relativistic as relativistic;
pub use amari_tropical as tropical;

#[cfg(feature = "measure")]
pub use amari_measure as measure;

#[cfg(feature = "calculus")]
pub use amari_calculus as calculus;

#[cfg(feature = "holographic")]
pub use amari_holographic as holographic;

#[cfg(feature = "probabilistic")]
pub use amari_probabilistic as probabilistic;

#[cfg(feature = "functional")]
pub use amari_functional as functional;

#[cfg(feature = "topology")]
pub use amari_topology as topology;

#[cfg(feature = "dynamics")]
pub use amari_dynamics as dynamics;

#[cfg(feature = "flynn")]
pub use amari_flynn as flynn;

#[cfg(feature = "gpu")]
pub use amari_gpu as gpu;

#[cfg(feature = "optimization")]
pub use amari_optimization as optimization;

// Deterministic computation module (opt-in via feature flag)
#[cfg(feature = "deterministic")]
pub mod deterministic;

use thiserror::Error;

/// Unified error type for the Amari library
#[derive(Error, Debug)]
pub enum AmariError {
    /// Core geometric algebra error
    #[error(transparent)]
    Core(#[from] amari_core::CoreError),

    /// Automata error
    #[error("{0}")]
    Automata(amari_automata::AutomataError),

    /// Enumerative geometry error
    #[error(transparent)]
    Enumerative(#[from] amari_enumerative::EnumerativeError),

    /// GPU computation error
    #[cfg(feature = "gpu")]
    #[error(transparent)]
    Gpu(#[from] amari_gpu::GpuError),

    /// Information geometry error
    #[error(transparent)]
    InfoGeom(#[from] amari_info_geom::InfoGeomError),

    /// Network analysis error
    #[error(transparent)]
    Network(#[from] amari_network::NetworkError),

    /// Fusion system error
    #[error(transparent)]
    Fusion(#[from] amari_fusion::EvaluationError),

    /// Tropical algebra error
    #[error(transparent)]
    Tropical(#[from] amari_tropical::TropicalError),

    /// Dual number error
    #[error(transparent)]
    Dual(#[from] amari_dual::DualError),

    /// Calculus error
    #[cfg(feature = "calculus")]
    #[error(transparent)]
    Calculus(#[from] amari_calculus::CalculusError),

    /// Holographic memory error
    #[cfg(feature = "holographic")]
    #[error(transparent)]
    Holographic(#[from] amari_holographic::HolographicError),

    /// Probabilistic error
    #[cfg(feature = "probabilistic")]
    #[error(transparent)]
    Probabilistic(#[from] amari_probabilistic::ProbabilisticError),

    /// Functional analysis error
    #[cfg(feature = "functional")]
    #[error(transparent)]
    Functional(#[from] amari_functional::FunctionalError),

    /// Topology error
    #[cfg(feature = "topology")]
    #[error(transparent)]
    Topology(#[from] amari_topology::TopologyError),

    /// Dynamics error
    #[cfg(feature = "dynamics")]
    #[error(transparent)]
    Dynamics(#[from] amari_dynamics::DynamicsError),
}

/// Result type for Amari operations
pub type AmariResult<T> = Result<T, AmariError>;

// Manual implementation for AutomataError since it doesn't use thiserror
impl From<amari_automata::AutomataError> for AmariError {
    fn from(err: amari_automata::AutomataError) -> Self {
        AmariError::Automata(err)
    }
}

// Re-export common types
pub use amari_core::{Bivector, Multivector, Scalar, Vector};
pub use amari_dual::{DualMultivector, DualNumber};
pub use amari_fusion::TropicalDualClifford;
pub use amari_info_geom::{DuallyFlatManifold, FisherInformationMatrix, SimpleAlphaConnection};
pub use amari_network::{
    Community, GeometricEdge, GeometricNetwork, NodeMetadata, PropagationAnalysis,
};
pub use amari_tropical::{TropicalMatrix, TropicalMultivector, TropicalNumber};
