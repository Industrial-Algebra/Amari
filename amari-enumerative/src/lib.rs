//! # Amari Enumerative Geometry
//!
//! This crate provides enumerative geometry capabilities for the Amari mathematical library.
//! It implements intersection theory, Schubert calculus, and tools for counting geometric
//! configurations such as curves, surfaces, and higher-dimensional varieties.
//!
//! ## Features
//!
//! - **Intersection Theory**: Chow rings, intersection multiplicities, and Bezout's theorem
//! - **Schubert Calculus**: Computations on Grassmannians and flag varieties
//! - **Littlewood-Richardson Coefficients**: Complete LR coefficient computation
//! - **Gromov-Witten Theory**: Curve counting and quantum cohomology
//! - **Tropical Geometry**: Tropical curve counting and correspondence theorems
//! - **Moduli Spaces**: Computations on moduli spaces of curves and surfaces
//! - **Namespace/Capabilities**: Geometric access control via Schubert calculus
//! - **CGT Bridge**: Growth-sequence adapters for `amari-cgt` layer reports (feature: `cgt-bridge`)
//! - **Phantom Types**: Compile-time verification of mathematical properties
//!
//! ## Usage
//!
//! ```rust
//! use amari_enumerative::{ProjectiveSpace, ChowClass, IntersectionRing};
//!
//! // Create projective 2-space
//! let p2 = ProjectiveSpace::new(2);
//!
//! // Define two curves
//! let cubic = ChowClass::hypersurface(3);
//! let quartic = ChowClass::hypersurface(4);
//!
//! // Compute intersection number (Bezout's theorem)
//! let intersection = p2.intersect(&cubic, &quartic);
//! assert_eq!(intersection.multiplicity(), 12); // 3 * 4 = 12
//! ```
//!
//! ## Schubert Calculus Example
//!
//! ```rust
//! use amari_enumerative::{SchubertCalculus, SchubertClass, IntersectionResult};
//!
//! // How many lines meet 4 general lines in projective 3-space?
//! let mut calc = SchubertCalculus::new((2, 4)); // Gr(2,4)
//! let sigma_1 = SchubertClass::new(vec![1], (2, 4)).unwrap();
//!
//! let classes = vec![sigma_1.clone(), sigma_1.clone(), sigma_1.clone(), sigma_1.clone()];
//! let result = calc.multi_intersect(&classes);
//!
//! assert_eq!(result, IntersectionResult::Finite(2)); // Answer: 2 lines!
//! ```
//!
//! ## Phantom Types for Compile-Time Verification
//!
//! The crate provides zero-cost phantom types for compile-time verification:
//!
//! - `ValidPartition` / `UnvalidatedPartition`: Partition validity states
//! - `Semistandard` / `LatticeWord`: Tableau property markers
//! - `Granted` / `Pending` / `Revoked`: Capability grant states
//! - `Transverse` / `Excess` / `Deficient`: Intersection dimension states
//! - `FitsInBox` / `UnverifiedBox`: Grassmannian containment states

#[cfg(test)]
pub mod comprehensive_tests;
pub mod verified_contracts;

#[cfg(feature = "cgt-bridge")]
pub mod cgt_bridge;
pub mod csm;
pub mod geometric_algebra;
pub mod gromov_witten;
pub mod higher_genus;
pub mod intersection;
pub mod littlewood_richardson;
pub mod localization;
pub mod matroid;
pub mod moduli_space;
pub mod namespace;
pub mod operad;
pub mod phantom;
pub mod schubert;
pub mod stability;
pub mod tropical_curves;
pub mod wdvv;

#[cfg(feature = "tropical-schubert")]
pub mod tropical_schubert;

#[cfg(feature = "gf2")]
pub mod finite_field;
pub mod kazhdan_lusztig;
#[cfg(feature = "gf2")]
pub mod representability;
#[cfg(feature = "gf2")]
pub mod weight_enumerator;

// Re-export core types
#[cfg(feature = "cgt-bridge")]
pub use cgt_bridge::{
    cgt_birthday_growth_summary, cgt_birthday_growth_summary_in, cgt_node_count_growth_summary,
    cgt_node_count_growth_summary_in, CgtEnumerationEntry, CgtEnumerationSummary,
};
#[cfg(feature = "tropical-schubert")]
pub use geometric_algebra::tropicalize_multivector;
pub use geometric_algebra::{
    quantum_k_theory, signatures, GeometricProjectiveSpace, GeometricSchubertClass,
    GeometricVariety,
};
pub use gromov_witten::{CurveClass as GWCurveClass, GromovWittenInvariant, QuantumCohomology};
pub use higher_genus::{
    AdvancedCurveCounting, DTInvariant, HigherGenusCurve, JacobianData, PTInvariant,
};
pub use intersection::{
    AlgebraicVariety, ChowClass, Constraint, Grassmannian, IntersectionNumber, IntersectionPoint,
    IntersectionRing, MockMultivector, ProjectiveSpace, QuantumProduct,
};
pub use moduli_space::{CurveClass, ModuliSpace, TautologicalClass};

// Schubert calculus exports
pub use schubert::{FlagVariety, IntersectionResult, SchubertCalculus, SchubertClass};

// Littlewood-Richardson exports
pub use littlewood_richardson::{
    lr_coefficient, schubert_product, Partition, SkewShape, SkewTableau,
};

// Parallel batch operations (when parallel feature is enabled)
#[cfg(feature = "parallel")]
pub use geometric_algebra::geometric_intersection_batch;
#[cfg(feature = "parallel")]
pub use littlewood_richardson::lr_coefficients_batch;
#[cfg(feature = "parallel")]
pub use namespace::{
    capability_accessible_batch, count_configurations_batch, namespace_intersection_batch,
};
#[cfg(feature = "parallel")]
pub use schubert::multi_intersect_batch;
#[cfg(feature = "parallel")]
pub use tropical_curves::{mikhalkin_correspondence_verify_batch, verify_mikhalkin_gw_batch};

// Namespace exports for geometric access control
pub use namespace::{
    capability_accessible, namespace_intersection, Capability, CapabilityId, Namespace,
    NamespaceBuilder, NamespaceError, NamespaceIntersection, QuantumCapability,
};

// Tropical curves
pub use tropical_curves::{
    verify_mikhalkin_gw, MikhalkinResult, TropicalCurve, TropicalEdge, TropicalIntersection,
    TropicalModuliSpace, TropicalPoint,
};

// Optional tropical Schubert exports
#[cfg(all(feature = "tropical-schubert", feature = "parallel"))]
pub use geometric_algebra::tropicalize_multivector_batch;
#[cfg(all(feature = "tropical-schubert", feature = "parallel"))]
pub use tropical_schubert::{tropical_convexity_batch, tropical_intersection_batch};
#[cfg(feature = "tropical-schubert")]
pub use tropical_schubert::{
    tropical_convexity_check, tropical_intersection_count, TropicalResult, TropicalSchubertClass,
};

// Phantom types for compile-time verification
pub use phantom::{
    // Box containment
    BoxContainment,
    Deficient,
    Excess,
    FitsInBox,
    // Grant states
    GrantState,
    Granted,
    // Intersection dimension
    IntersectionDimension,
    LatticeWord,
    // Partition validity
    PartitionValidity,
    Pending,
    // Properties wrapper
    Properties,
    Revoked,
    Semistandard,
    // Tableau validity
    TableauValidity,
    Transverse,
    UnknownDimension,
    UnvalidatedPartition,
    UnverifiedBox,
    UnverifiedTableau,
    // Type aliases
    ValidLRTableau,
    ValidPartition,
    ValidSchubertClass,
};

use thiserror::Error;

/// Error types for enumerative geometry computations
#[derive(Error, Debug, Clone, PartialEq)]
pub enum EnumerativeError {
    /// Invalid dimension for the ambient space
    #[error("Invalid dimension: {0}")]
    InvalidDimension(String),

    /// `amari-cgt` bridge error
    #[cfg(feature = "cgt-bridge")]
    #[error(transparent)]
    CgtBridge(#[from] amari_cgt::CgtError),

    /// Intersection computation failed
    #[error("Intersection error: {0}")]
    IntersectionError(String),

    /// Schubert calculus error
    #[error("Schubert calculus error: {0}")]
    SchubertError(String),

    /// Gromov-Witten invariant computation error
    #[error("Gromov-Witten error: {0}")]
    GromovWittenError(String),

    /// General computational error
    #[error("Computation error: {0}")]
    ComputationError(String),

    /// Matroid representability error
    #[error("Representability error: {0}")]
    RepresentabilityError(String),

    /// Linear code error
    #[error("Code error: {0}")]
    CodeError(String),

    /// Invalid field size (must be a prime power)
    #[error("Invalid field size: {q} is not a prime power")]
    InvalidFieldSize { q: u64 },

    /// Search space too large for exhaustive computation
    #[error("Search space too large: {description}")]
    SearchSpaceTooLarge { description: String },
}

/// Result type for enumerative geometry computations
pub type EnumerativeResult<T> = Result<T, EnumerativeError>;

// WDVV recursion
pub use wdvv::WDVVEngine;

// Equivariant localization
pub use localization::{EquivariantLocalizer, FixedPoint, TorusWeights};

// Matroids
pub use matroid::{Matroid, ValuatedMatroid};

// Kazhdan-Lusztig polynomials
pub use kazhdan_lusztig::{
    flag_f_vector, inverse_kl_polynomial, kl_is_non_negative, kl_polynomial, z_polynomial,
};

// GF(2)-dependent modules
#[cfg(feature = "gf2")]
pub use finite_field::{
    grassmannian_poincare_polynomial, grassmannian_points, point_counts_over_extensions,
    schubert_cell_points, schubert_poincare_polynomial, schubert_variety_points,
    schubert_zeta_exponents,
};
#[cfg(feature = "gf2")]
pub use representability::{
    column_matroid, dual_fano_matroid, fano_matroid, has_minor, is_binary, is_regular, is_ternary,
    standard_representation, RepresentabilityResult,
};
#[cfg(feature = "gf2")]
pub use weight_enumerator::{
    extended_golay_code, gilbert_varshamov_bound, hamming_bound, hamming_code, plotkin_bound,
    reed_muller_code, simplex_code, singleton_bound, BinaryCode,
};

// CSM classes
pub use csm::{CSMClass, SegreClass};

// Operadic composition
pub use operad::{
    compose_namespaces, composition_multiplicity, interfaces_compatible, ComposableNamespace,
    Interface, InterfaceDirection,
};

// Wall-crossing / stability
pub use stability::{StabilityCondition, Wall, WallCrossingEngine};

// Parallel batch operations for new modules
#[cfg(feature = "parallel")]
pub use csm::{csm_of_cells_batch, euler_characteristic_batch};
#[cfg(feature = "parallel")]
pub use matroid::{circuits_batch, intersection_cardinality_batch, tutte_polynomial_batch};
#[cfg(feature = "parallel")]
pub use stability::{compute_walls_batch, stable_count_batch};
#[cfg(feature = "parallel")]
pub use wdvv::rational_curve_count_batch;
