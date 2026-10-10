//! Stability analysis for dynamical systems on geometric algebra spaces.
//!
//! This module provides comprehensive tools for analyzing the stability of
//! fixed points in dynamical systems defined on Clifford algebras Cl(P,Q,R).
//!
//! # Mathematical Framework
//!
//! Stability analysis in geometric algebra connects classical dynamical systems
//! theory with the rich algebraic structure of Clifford algebras:
//!
//! - **Fixed points** are found where the vector field vanishes: f(x*) = 0
//! - **Linearization** uses the Jacobian, which can be expressed as geometric
//!   derivatives when the vector field has a geometric interpretation
//! - **Eigenvalues** of the Jacobian determine local stability, and in GA
//!   settings may have interpretations related to rotations (bivector parts)
//!   and dilations (scalar parts)
//!
//! # Stability Classification
//!
//! For continuous-time systems dx/dt = f(x):
//!
//! | Stability Type | Eigenvalue Condition | Geometric Interpretation |
//! |---------------|---------------------|-------------------------|
//! | Stable Node | All λ < 0, real | Direct contraction |
//! | Stable Focus | Re(λ) < 0, complex | Spiral contraction |
//! | Unstable Node | All λ > 0, real | Direct expansion |
//! | Unstable Focus | Re(λ) > 0, complex | Spiral expansion |
//! | Saddle | Mixed signs | Hyperbolic splitting |
//! | Center | Re(λ) = 0 | Conservative oscillation |
//!
//! # Integration with amari-functional
//!
//! The stability module leverages amari-functional's spectral theory:
//! - Eigenvalue computation via spectral decomposition
//! - Operator norms for condition number analysis
//! - Hilbert space structure for eigenvector orthogonality
//!
//! # Example
//!
//! ```ignore
//! use amari_dynamics::stability::{
//!     find_fixed_point, analyze_stability, FixedPointConfig, DifferentiationConfig
//! };
//! use amari_core::Multivector;
//!
//! // Define a system (harmonic oscillator)
//! let system = HarmonicOscillator::new(1.0);
//!
//! // Find fixed point
//! let initial = Multivector::<2, 0, 0>::zero();
//! let fp = find_fixed_point(&system, &initial, &FixedPointConfig::default())?;
//!
//! // Analyze stability
//! let analysis = analyze_stability(
//!     &system,
//!     &fp.point,
//!     &DifferentiationConfig::default(),
//!     1e-10
//! )?;
//!
//! println!("Stability: {}", analysis.stability);
//! println!("Eigenvalues: {:?}", analysis.eigenvalues);
//! ```
//!
//! # Geometric Algebra Perspective
//!
//! In geometric algebra, the Jacobian matrix can be viewed through the lens of
//! multivector derivatives. For a multivector field F: Cl(P,Q,R) → Cl(P,Q,R),
//! the derivative dF/dx can be decomposed into:
//!
//! - **Scalar part**: Divergence-like behavior (trace of Jacobian)
//! - **Bivector part**: Rotational components (antisymmetric part)
//! - **Higher grades**: More complex couplings
//!
//! The eigenvalue structure reflects this decomposition:
//! - Real eigenvalues correspond to purely expanding/contracting modes
//! - Complex eigenvalues indicate rotational (bivector-like) dynamics
//! - The spectral abscissa determines asymptotic stability

pub mod classification;
pub mod eigenvalue;
pub mod fixed_point;
pub mod linearization;

// Re-export main types
pub use classification::{
    classify_discrete_from_eigenvalues, classify_from_eigenvalues, DiscreteStabilityType,
    StabilityType,
};

pub use eigenvalue::{
    analyze_discrete_stability, analyze_stability, characteristic_polynomial, compute_eigenvalues,
    compute_subspace_dimensions, hopf_bifurcation_check, saddle_node_check,
    DiscreteEigenvalueAnalysis, EigenvalueAnalysis, SubspaceDimensions,
};

pub use fixed_point::{
    find_fixed_point, find_fixed_points, generate_initial_conditions, FixedPointConfig,
    FixedPointResult,
};

pub use linearization::{
    compute_hessian, compute_jacobian, divergence, is_gradient, is_volume_preserving, linearize,
    DifferentiationConfig, LinearizedSystem,
};

#[cfg(feature = "parallel")]
pub use parallel::*;

/// Parallel implementations of stability analysis
#[cfg(feature = "parallel")]
mod parallel {
    use super::*;
    use crate::error::Result;
    use crate::flow::DynamicalSystem;
    use amari_core::Multivector;
    use rayon::prelude::*;

    /// Find fixed points from multiple initial conditions in parallel
    ///
    /// This is particularly useful for systems with multiple fixed points
    /// where a global search is needed.
    pub fn find_fixed_points_parallel<S, const P: usize, const Q: usize, const R: usize>(
        system: &S,
        initial_conditions: &[Multivector<P, Q, R>],
        config: &FixedPointConfig,
        merge_tolerance: f64,
    ) -> Result<Vec<FixedPointResult<P, Q, R>>>
    where
        S: DynamicalSystem<P, Q, R> + Sync,
    {
        // Find fixed points in parallel
        let results: Vec<_> = initial_conditions
            .par_iter()
            .filter_map(|initial| {
                find_fixed_point(system, initial, config)
                    .ok()
                    .filter(|r| r.converged)
            })
            .collect();

        // Merge duplicates (sequential to maintain determinism)
        let mut unique = Vec::new();
        for result in results {
            let is_new = unique.iter().all(|fp: &FixedPointResult<P, Q, R>| {
                let diff = &result.point - &fp.point;
                diff.coefficient_norm() > merge_tolerance
            });
            if is_new {
                unique.push(result);
            }
        }

        Ok(unique)
    }

    /// Analyze stability at multiple points in parallel
    pub fn analyze_stability_batch<S, const P: usize, const Q: usize, const R: usize>(
        system: &S,
        points: &[Multivector<P, Q, R>],
        config: &DifferentiationConfig,
        tolerance: f64,
    ) -> Vec<Result<EigenvalueAnalysis>>
    where
        S: DynamicalSystem<P, Q, R> + Sync,
    {
        points
            .par_iter()
            .map(|point| analyze_stability(system, point, config, tolerance))
            .collect()
    }
}

/// Convenience function combining fixed point finding and stability analysis
///
/// This function finds a fixed point and immediately analyzes its stability.
pub fn find_and_analyze<S, const P: usize, const Q: usize, const R: usize>(
    system: &S,
    initial: &amari_core::Multivector<P, Q, R>,
    fp_config: &FixedPointConfig,
    diff_config: &DifferentiationConfig,
    tolerance: f64,
) -> crate::error::Result<(FixedPointResult<P, Q, R>, EigenvalueAnalysis)>
where
    S: crate::flow::DynamicalSystem<P, Q, R>,
{
    let fp_result = find_fixed_point(system, initial, fp_config)?;

    if !fp_result.converged {
        return Err(crate::error::DynamicsError::convergence_failure(
            fp_config.max_iterations,
            format!(
                "Fixed point finding did not converge (residual: {:.2e})",
                fp_result.residual
            ),
        ));
    }

    let analysis = analyze_stability(system, &fp_result.point, diff_config, tolerance)?;

    Ok((fp_result, analysis))
}

/// Full stability report for a fixed point
#[derive(Debug, Clone)]
pub struct StabilityReport<const P: usize, const Q: usize, const R: usize> {
    /// The fixed point
    pub fixed_point: FixedPointResult<P, Q, R>,
    /// Eigenvalue analysis
    pub analysis: EigenvalueAnalysis,
    /// Subspace dimensions
    pub dimensions: SubspaceDimensions,
    /// Whether near a Hopf bifurcation
    pub near_hopf: Option<(f64, f64)>,
    /// Whether near a saddle-node bifurcation
    pub near_saddle_node: Option<f64>,
}

impl<const P: usize, const Q: usize, const R: usize> StabilityReport<P, Q, R> {
    /// Check if the fixed point is hyperbolic (structurally stable)
    pub fn is_hyperbolic(&self) -> bool {
        self.dimensions.is_hyperbolic()
    }

    /// Check if near any bifurcation
    pub fn near_bifurcation(&self) -> bool {
        self.near_hopf.is_some() || self.near_saddle_node.is_some()
    }
}

/// Generate a complete stability report for a fixed point
pub fn stability_report<S, const P: usize, const Q: usize, const R: usize>(
    system: &S,
    initial: &amari_core::Multivector<P, Q, R>,
    fp_config: &FixedPointConfig,
    diff_config: &DifferentiationConfig,
    tolerance: f64,
) -> crate::error::Result<StabilityReport<P, Q, R>>
where
    S: crate::flow::DynamicalSystem<P, Q, R>,
{
    let (fixed_point, analysis) =
        find_and_analyze(system, initial, fp_config, diff_config, tolerance)?;

    let dimensions = compute_subspace_dimensions(&analysis, tolerance);
    let near_hopf = hopf_bifurcation_check(&analysis, tolerance);
    let near_saddle_node = saddle_node_check(&analysis, tolerance);

    Ok(StabilityReport {
        fixed_point,
        analysis,
        dimensions,
        near_hopf,
        near_saddle_node,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flow::HarmonicOscillator;
    use amari_core::Multivector;

    #[test]
    fn test_find_and_analyze() {
        let system = HarmonicOscillator::new(1.0);
        let initial = Multivector::<2, 0, 0>::zero();

        let (fp, analysis) = find_and_analyze(
            &system,
            &initial,
            &FixedPointConfig::default(),
            &DifferentiationConfig::default(),
            1e-10,
        )
        .unwrap();

        assert!(fp.converged);
        assert_eq!(analysis.stability, StabilityType::Center);
    }

    #[test]
    fn test_stability_report() {
        let system = HarmonicOscillator::new(1.0);
        let initial = Multivector::<2, 0, 0>::zero();

        let report = stability_report(
            &system,
            &initial,
            &FixedPointConfig::default(),
            &DifferentiationConfig::default(),
            1e-10,
        )
        .unwrap();

        assert!(report.fixed_point.converged);
        // Harmonic oscillator is a center, which has center manifold
        assert!(report.dimensions.center > 0);
        // Should detect near-Hopf condition
        assert!(report.near_hopf.is_some());
    }
}
