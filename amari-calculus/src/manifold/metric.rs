//! Metric tensor implementation for Riemannian manifolds

use crate::ScalarField;

/// Metric tensor g_ij defining the geometry of a Riemannian manifold
///
/// The metric tensor encodes distances and angles on a curved space:
/// ds² = g_ij dx^i dx^j
#[derive(Clone)]
pub struct MetricTensor<const DIM: usize> {
    /// Metric components g_ij as functions of coordinates
    components: [[ScalarField<3, 0, 0>; DIM]; DIM],
}

impl<const DIM: usize> MetricTensor<DIM> {
    /// Create a metric tensor from component functions
    ///
    /// # Arguments
    ///
    /// * `components` - 2D array of scalar fields g_ij(x)
    pub fn new(components: [[ScalarField<3, 0, 0>; DIM]; DIM]) -> Self {
        Self { components }
    }

    /// Get metric component g_ij at a point
    ///
    /// # Arguments
    ///
    /// * `i` - First index
    /// * `j` - Second index
    /// * `coords` - Point at which to evaluate
    pub fn component(&self, i: usize, j: usize, coords: &[f64]) -> f64 {
        self.components[i][j].evaluate(coords)
    }

    /// Compute inverse metric g^ij at a point
    ///
    /// For 2D, uses explicit formula. For higher dimensions,
    /// would need matrix inversion.
    pub fn inverse(&self, coords: &[f64]) -> [[f64; DIM]; DIM] {
        let mut inv = [[0.0; DIM]; DIM];

        if DIM == 2 {
            // Explicit 2x2 inverse
            let g00 = self.component(0, 0, coords);
            let g01 = self.component(0, 1, coords);
            let g10 = self.component(1, 0, coords);
            let g11 = self.component(1, 1, coords);

            let det = g00 * g11 - g01 * g10;
            if det.abs() > 1e-10 {
                inv[0][0] = g11 / det;
                inv[0][1] = -g01 / det;
                inv[1][0] = -g10 / det;
                inv[1][1] = g00 / det;
            }
        } else if DIM == 3 {
            // Explicit 3x3 inverse
            let g = [
                [
                    self.component(0, 0, coords),
                    self.component(0, 1, coords),
                    self.component(0, 2, coords),
                ],
                [
                    self.component(1, 0, coords),
                    self.component(1, 1, coords),
                    self.component(1, 2, coords),
                ],
                [
                    self.component(2, 0, coords),
                    self.component(2, 1, coords),
                    self.component(2, 2, coords),
                ],
            ];

            // Compute determinant
            let det = g[0][0] * (g[1][1] * g[2][2] - g[1][2] * g[2][1])
                - g[0][1] * (g[1][0] * g[2][2] - g[1][2] * g[2][0])
                + g[0][2] * (g[1][0] * g[2][1] - g[1][1] * g[2][0]);

            if det.abs() > 1e-10 {
                inv[0][0] = (g[1][1] * g[2][2] - g[1][2] * g[2][1]) / det;
                inv[0][1] = (g[0][2] * g[2][1] - g[0][1] * g[2][2]) / det;
                inv[0][2] = (g[0][1] * g[1][2] - g[0][2] * g[1][1]) / det;
                inv[1][0] = (g[1][2] * g[2][0] - g[1][0] * g[2][2]) / det;
                inv[1][1] = (g[0][0] * g[2][2] - g[0][2] * g[2][0]) / det;
                inv[1][2] = (g[0][2] * g[1][0] - g[0][0] * g[1][2]) / det;
                inv[2][0] = (g[1][0] * g[2][1] - g[1][1] * g[2][0]) / det;
                inv[2][1] = (g[0][1] * g[2][0] - g[0][0] * g[2][1]) / det;
                inv[2][2] = (g[0][0] * g[1][1] - g[0][1] * g[1][0]) / det;
            }
        }

        inv
    }

    /// Create Euclidean (flat) metric: g_ij = δ_ij
    pub fn euclidean() -> Self
    where
        [(); DIM]:,
    {
        // Manually create components to avoid Copy requirement
        let zero = ScalarField::new(|_| 0.0);
        let one = ScalarField::new(|_| 1.0);

        // For 2D and 3D, create components manually
        let components = if DIM == 2 {
            let comps: [[ScalarField<3, 0, 0>; 2]; 2] =
                [[one.clone(), zero.clone()], [zero.clone(), one.clone()]];

            // SAFETY (M-REP-D S5): `transmute_copy` reinterprets a
            // `[[ScalarField<3,0,0>; N]; N]` as `[[ScalarField; DIM]; DIM]`.
            // - Sizes: the branch guard pins N == DIM, so element counts and
            //   layouts match exactly (transmute_copy's own size check would
            //   panic otherwise; it is a runtime check, not compile-time).
            // - Ownership: ScalarField is `{ fn(&[f64]) -> f64, usize }` —
            //   no heap, no drop glue — so the bitwise duplication cannot
            //   double-drop. Adding any owning field to ScalarField makes
            //   this site UB; the invariant is asserted by the S5 audit.
            unsafe { std::mem::transmute_copy(&comps) }
        } else if DIM == 3 {
            let comps: [[ScalarField<3, 0, 0>; 3]; 3] = [
                [one.clone(), zero.clone(), zero.clone()],
                [zero.clone(), one.clone(), zero.clone()],
                [zero.clone(), zero.clone(), one.clone()],
            ];

            // SAFETY (M-REP-D S5): `transmute_copy` reinterprets a
            // `[[ScalarField<3,0,0>; N]; N]` as `[[ScalarField; DIM]; DIM]`.
            // - Sizes: the branch guard pins N == DIM, so element counts and
            //   layouts match exactly (transmute_copy's own size check would
            //   panic otherwise; it is a runtime check, not compile-time).
            // - Ownership: ScalarField is `{ fn(&[f64]) -> f64, usize }` —
            //   no heap, no drop glue — so the bitwise duplication cannot
            //   double-drop. Adding any owning field to ScalarField makes
            //   this site UB; the invariant is asserted by the S5 audit.
            unsafe { std::mem::transmute_copy(&comps) }
        } else {
            panic!("Euclidean metric only implemented for DIM=2 and DIM=3");
        };

        Self { components }
    }

    /// Create metric for 2-sphere of radius R in (θ, φ) coordinates
    ///
    /// ds² = R²(dθ² + sin²θ dφ²)
    ///
    /// Only valid for DIM=2.
    ///
    /// Note: Due to limitations with function pointers, radius is fixed at 1.0.
    /// For other radii, scale coordinates appropriately.
    pub fn sphere(_radius: f64) -> Self
    where
        [(); DIM]:,
    {
        assert_eq!(DIM, 2, "Sphere metric only defined for DIM=2");

        // Note: We can't capture radius in closures, so we use r=1
        // Users can scale coordinates to get different radii
        let components_2d: [[ScalarField<3, 0, 0>; 2]; 2] = [
            [
                ScalarField::with_dimension(|_| 1.0, 2),
                ScalarField::with_dimension(|_| 0.0, 2),
            ],
            [
                ScalarField::with_dimension(|_| 0.0, 2),
                ScalarField::with_dimension(|coords| coords[0].sin().powi(2), 2),
            ],
        ];

        // SAFETY: Safe because we checked DIM == 2
        let components =
            // SAFETY (M-REP-D S5): as above — N == DIM branch guard, matching
            // layout, ScalarField has no owning state or drop glue.
            unsafe { std::mem::transmute_copy(&components_2d) };

        Self { components }
    }

    /// Create metric for hyperbolic plane (Poincaré disk model)
    ///
    /// ds² = 4(dx² + dy²)/(1 - x² - y²)²
    ///
    /// Only valid for DIM=2.
    pub fn hyperbolic() -> Self
    where
        [(); DIM]:,
    {
        assert_eq!(DIM, 2, "Hyperbolic metric only defined for DIM=2");

        let components_2d: [[ScalarField<3, 0, 0>; 2]; 2] = [
            [
                ScalarField::with_dimension(
                    |coords| {
                        let x = coords[0];
                        let y = coords[1];
                        let denom = 1.0 - x * x - y * y;
                        4.0 / (denom * denom)
                    },
                    2,
                ),
                ScalarField::with_dimension(|_| 0.0, 2),
            ],
            [
                ScalarField::with_dimension(|_| 0.0, 2),
                ScalarField::with_dimension(
                    |coords| {
                        let x = coords[0];
                        let y = coords[1];
                        let denom = 1.0 - x * x - y * y;
                        4.0 / (denom * denom)
                    },
                    2,
                ),
            ],
        ];

        // SAFETY: Safe because we checked DIM == 2
        let components =
            // SAFETY (M-REP-D S5): as above — N == DIM branch guard, matching
            // layout, ScalarField has no owning state or drop glue.
            unsafe { std::mem::transmute_copy(&components_2d) };

        Self { components }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_euclidean_metric() {
        let metric = MetricTensor::<2>::euclidean();

        // Check diagonal is 1, off-diagonal is 0
        assert!((metric.component(0, 0, &[0.0, 0.0]) - 1.0).abs() < 1e-10);
        assert!((metric.component(1, 1, &[0.0, 0.0]) - 1.0).abs() < 1e-10);
        assert!(metric.component(0, 1, &[0.0, 0.0]).abs() < 1e-10);
        assert!(metric.component(1, 0, &[0.0, 0.0]).abs() < 1e-10);
    }

    #[test]
    fn test_sphere_metric() {
        let metric = MetricTensor::<2>::sphere(1.0);

        // At θ=π/2 (equator), metric should be diag(1, 1) for unit sphere
        let theta = std::f64::consts::PI / 2.0;
        let phi = 0.0;

        assert!((metric.component(0, 0, &[theta, phi]) - 1.0).abs() < 1e-6);
        assert!((metric.component(1, 1, &[theta, phi]) - 1.0).abs() < 1e-6);
        assert!(metric.component(0, 1, &[theta, phi]).abs() < 1e-10);
    }

    #[test]
    fn test_metric_inverse() {
        let metric = MetricTensor::<2>::euclidean();
        let inv = metric.inverse(&[0.0, 0.0]);

        // Euclidean inverse is also identity
        assert!((inv[0][0] - 1.0).abs() < 1e-10);
        assert!((inv[1][1] - 1.0).abs() < 1e-10);
        assert!(inv[0][1].abs() < 1e-10);
        assert!(inv[1][0].abs() < 1e-10);
    }
}
