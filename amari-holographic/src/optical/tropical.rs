//! Tropical operations on optical rotor fields.
//!
//! Extends `OpticalFieldAlgebra` with tropical semiring operations
//! for attractor dynamics and shortest-path computations.

use super::algebra::OpticalFieldAlgebra;
use super::rotor_field::OpticalRotorField;

/// Tropical operations on optical fields.
///
/// The tropical semiring uses (min, +) operations instead of (+, ×).
/// This is useful for:
/// - Attractor dynamics (finding minimum-distance states)
/// - Shortest-path computations on phase space
/// - Winner-take-all operations
///
/// # Example
///
/// ```ignore
/// use amari_holographic::optical::{TropicalOpticalAlgebra, OpticalRotorField};
///
/// let tropical = TropicalOpticalAlgebra::new((64, 64));
///
/// let field_a = OpticalRotorField::random((64, 64), 1);
/// let field_b = OpticalRotorField::random((64, 64), 2);
///
/// // Tropical add: point-wise minimum phase magnitude
/// let result = tropical.tropical_add(&field_a, &field_b);
/// ```
pub struct TropicalOpticalAlgebra {
    inner: OpticalFieldAlgebra,
}

impl TropicalOpticalAlgebra {
    /// Create a new tropical algebra for fields of the given dimensions.
    pub fn new(dimensions: (usize, usize)) -> Self {
        Self {
            inner: OpticalFieldAlgebra::new(dimensions),
        }
    }

    /// Access the underlying optical field algebra.
    pub fn inner(&self) -> &OpticalFieldAlgebra {
        &self.inner
    }

    /// Get the dimensions.
    pub fn dimensions(&self) -> (usize, usize) {
        self.inner.dimensions()
    }

    /// Tropical addition: point-wise minimum of phase magnitudes.
    ///
    /// For each pixel, selects the rotor with smaller absolute phase.
    /// This corresponds to the "nearest to identity" rotor at each point.
    ///
    /// Useful for computing "nearest" in phase space.
    pub fn tropical_add(&self, a: &OpticalRotorField, b: &OpticalRotorField) -> OpticalRotorField {
        assert_eq!(a.dimensions(), b.dimensions());

        let n = a.len();
        let mut scalar = Vec::with_capacity(n);
        let mut bivector = Vec::with_capacity(n);
        let mut amplitude = Vec::with_capacity(n);

        for i in 0..n {
            let phase_a = a.bivectors()[i].atan2(a.scalars()[i]).abs();
            let phase_b = b.bivectors()[i].atan2(b.scalars()[i]).abs();

            if phase_a <= phase_b {
                scalar.push(a.scalars()[i]);
                bivector.push(a.bivectors()[i]);
                amplitude.push(a.amplitudes()[i]);
            } else {
                scalar.push(b.scalars()[i]);
                bivector.push(b.bivectors()[i]);
                amplitude.push(b.amplitudes()[i]);
            }
        }

        OpticalRotorField {
            scalar,
            bivector,
            amplitude,
            dimensions: a.dimensions(),
        }
    }

    /// Tropical maximum: point-wise maximum of phase magnitudes.
    ///
    /// For each pixel, selects the rotor with larger absolute phase.
    /// This is the dual operation to tropical_add.
    pub fn tropical_max(&self, a: &OpticalRotorField, b: &OpticalRotorField) -> OpticalRotorField {
        assert_eq!(a.dimensions(), b.dimensions());

        let n = a.len();
        let mut scalar = Vec::with_capacity(n);
        let mut bivector = Vec::with_capacity(n);
        let mut amplitude = Vec::with_capacity(n);

        for i in 0..n {
            let phase_a = a.bivectors()[i].atan2(a.scalars()[i]).abs();
            let phase_b = b.bivectors()[i].atan2(b.scalars()[i]).abs();

            if phase_a >= phase_b {
                scalar.push(a.scalars()[i]);
                bivector.push(a.bivectors()[i]);
                amplitude.push(a.amplitudes()[i]);
            } else {
                scalar.push(b.scalars()[i]);
                bivector.push(b.bivectors()[i]);
                amplitude.push(b.amplitudes()[i]);
            }
        }

        OpticalRotorField {
            scalar,
            bivector,
            amplitude,
            dimensions: a.dimensions(),
        }
    }

    /// Tropical multiplication: binding operation from the inner algebra.
    ///
    /// In tropical semiring terms, this is the "additive" operation
    /// that corresponds to path concatenation.
    pub fn tropical_mul(&self, a: &OpticalRotorField, b: &OpticalRotorField) -> OpticalRotorField {
        self.inner.bind(a, b)
    }

    /// Find the minimum phase field from a collection.
    ///
    /// Returns the field that is "closest to identity" at each pixel
    /// (tropical sum over all fields).
    pub fn tropical_sum<'a>(
        &self,
        fields: impl IntoIterator<Item = &'a OpticalRotorField>,
    ) -> Option<OpticalRotorField> {
        let mut iter = fields.into_iter();
        let mut result = iter.next()?.clone();

        for field in iter {
            result = self.tropical_add(&result, field);
        }

        Some(result)
    }

    /// Find the maximum phase field from a collection.
    pub fn tropical_max_all<'a>(
        &self,
        fields: impl IntoIterator<Item = &'a OpticalRotorField>,
    ) -> Option<OpticalRotorField> {
        let mut iter = fields.into_iter();
        let mut result = iter.next()?.clone();

        for field in iter {
            result = self.tropical_max(&result, field);
        }

        Some(result)
    }

    /// Compute phase distance between two fields.
    ///
    /// Returns the sum of absolute phase differences.
    pub fn phase_distance(&self, a: &OpticalRotorField, b: &OpticalRotorField) -> f32 {
        assert_eq!(a.dimensions(), b.dimensions());

        let mut distance = 0.0f32;
        for i in 0..a.len() {
            let phase_a = a.bivectors()[i].atan2(a.scalars()[i]);
            let phase_b = b.bivectors()[i].atan2(b.scalars()[i]);
            let diff = (phase_a - phase_b).abs();
            // Handle wrap-around
            let wrapped = diff.min(std::f32::consts::TAU - diff);
            distance += wrapped;
        }

        distance
    }

    /// Compute normalized phase distance (average per pixel).
    pub fn normalized_phase_distance(&self, a: &OpticalRotorField, b: &OpticalRotorField) -> f32 {
        if a.is_empty() {
            return 0.0;
        }
        self.phase_distance(a, b) / a.len() as f32
    }

    /// Soft tropical addition using logsumexp-style smoothing.
    ///
    /// Instead of hard min, uses softmin with temperature parameter `beta`:
    /// - Large `beta` → hard minimum (standard tropical)
    /// - Small `beta` → soft average
    pub fn soft_tropical_add(
        &self,
        a: &OpticalRotorField,
        b: &OpticalRotorField,
        beta: f32,
    ) -> OpticalRotorField {
        assert_eq!(a.dimensions(), b.dimensions());

        let n = a.len();
        let mut scalar = Vec::with_capacity(n);
        let mut bivector = Vec::with_capacity(n);
        let mut amplitude = Vec::with_capacity(n);

        for i in 0..n {
            let phase_a = a.bivectors()[i].atan2(a.scalars()[i]).abs();
            let phase_b = b.bivectors()[i].atan2(b.scalars()[i]).abs();

            // Softmin using exp(-beta * phase)
            let exp_a = (-beta * phase_a).exp();
            let exp_b = (-beta * phase_b).exp();
            let sum = exp_a + exp_b;

            // Weights for soft interpolation
            let w_a = exp_a / sum;
            let w_b = exp_b / sum;

            // Interpolate the rotor components
            let s = w_a * a.scalars()[i] + w_b * b.scalars()[i];
            let bv = w_a * a.bivectors()[i] + w_b * b.bivectors()[i];
            let amp = w_a * a.amplitudes()[i] + w_b * b.amplitudes()[i];

            // Re-normalize the rotor part
            let r = (s * s + bv * bv).sqrt();
            if r > 1e-10 {
                scalar.push(s / r);
                bivector.push(bv / r);
            } else {
                scalar.push(1.0);
                bivector.push(0.0);
            }
            amplitude.push(amp);
        }

        OpticalRotorField {
            scalar,
            bivector,
            amplitude,
            dimensions: a.dimensions(),
        }
    }

    /// Apply tropical iteration for attractor dynamics.
    ///
    /// Given a current state and a set of attractor fields, computes
    /// one iteration of tropical dynamics:
    ///
    /// `new_state = tropical_add(attractors[0], tropical_add(attractors[1], ...))`
    ///
    /// This drives the state toward the "nearest" attractor at each pixel.
    pub fn attractor_step(
        &self,
        _current: &OpticalRotorField,
        attractors: &[OpticalRotorField],
    ) -> Option<OpticalRotorField> {
        if attractors.is_empty() {
            return None;
        }

        self.tropical_sum(attractors.iter())
    }

    /// Iterate attractor dynamics until convergence.
    ///
    /// # Arguments
    ///
    /// * `initial` - Starting state
    /// * `attractors` - Set of attractor fields
    /// * `max_iterations` - Maximum number of iterations
    /// * `tolerance` - Convergence tolerance for phase distance
    ///
    /// # Returns
    ///
    /// Final state and number of iterations taken.
    pub fn attractor_converge(
        &self,
        initial: &OpticalRotorField,
        attractors: &[OpticalRotorField],
        max_iterations: usize,
        tolerance: f32,
    ) -> (OpticalRotorField, usize) {
        let mut current = initial.clone();

        for i in 0..max_iterations {
            let next = match self.attractor_step(&current, attractors) {
                Some(n) => n,
                None => return (current, i),
            };

            let dist = self.normalized_phase_distance(&current, &next);
            current = next;

            if dist < tolerance {
                return (current, i + 1);
            }
        }

        (current, max_iterations)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, PI};

    fn approx_eq(a: f32, b: f32, eps: f32) -> bool {
        (a - b).abs() < eps
    }

    #[test]
    fn test_tropical_add_selects_smaller_phase() {
        let tropical = TropicalOpticalAlgebra::new((4, 1));

        // Field a: phases [0, π/4, π/2, π]
        let field_a = OpticalRotorField::from_phase(vec![0.0, FRAC_PI_4, FRAC_PI_2, PI], (4, 1));

        // Field b: phases [π/4, π/4, π/4, π/4]
        let field_b =
            OpticalRotorField::from_phase(vec![FRAC_PI_4, FRAC_PI_4, FRAC_PI_4, FRAC_PI_4], (4, 1));

        let result = tropical.tropical_add(&field_a, &field_b);

        // Should select smaller |phase| at each point:
        // i=0: |0| < |π/4|, select a (phase 0)
        // i=1: |π/4| == |π/4|, select a (tie goes to a)
        // i=2: |π/2| > |π/4|, select b (phase π/4)
        // i=3: |π| > |π/4|, select b (phase π/4)

        assert!(approx_eq(result.phase_at(0, 0), 0.0, 1e-5));
        assert!(approx_eq(result.phase_at(1, 0), FRAC_PI_4, 1e-5));
        assert!(approx_eq(result.phase_at(2, 0), FRAC_PI_4, 1e-5));
        assert!(approx_eq(result.phase_at(3, 0), FRAC_PI_4, 1e-5));
    }

    #[test]
    fn test_tropical_max_selects_larger_phase() {
        let tropical = TropicalOpticalAlgebra::new((4, 1));

        let field_a = OpticalRotorField::from_phase(vec![0.0, FRAC_PI_4, FRAC_PI_2, PI], (4, 1));
        let field_b =
            OpticalRotorField::from_phase(vec![FRAC_PI_4, FRAC_PI_4, FRAC_PI_4, FRAC_PI_4], (4, 1));

        let result = tropical.tropical_max(&field_a, &field_b);

        // Should select larger |phase| at each point
        assert!(approx_eq(result.phase_at(0, 0), FRAC_PI_4, 1e-5));
        assert!(approx_eq(result.phase_at(1, 0), FRAC_PI_4, 1e-5));
        assert!(approx_eq(result.phase_at(2, 0), FRAC_PI_2, 1e-5));
        // For |π| vs |π/4|, π wins. Note: atan2 may return -π or π
        assert!(approx_eq(result.phase_at(3, 0).abs(), PI, 1e-5));
    }

    #[test]
    fn test_tropical_mul_is_binding() {
        let tropical = TropicalOpticalAlgebra::new((8, 8));

        let field_a = OpticalRotorField::random((8, 8), 1);
        let field_b = OpticalRotorField::random((8, 8), 2);

        let trop_mul = tropical.tropical_mul(&field_a, &field_b);
        let inner_bind = tropical.inner().bind(&field_a, &field_b);

        // Should be identical
        assert_eq!(trop_mul.scalars(), inner_bind.scalars());
        assert_eq!(trop_mul.bivectors(), inner_bind.bivectors());
    }

    #[test]
    fn test_phase_distance() {
        let tropical = TropicalOpticalAlgebra::new((4, 1));

        let field_a = OpticalRotorField::from_phase(vec![0.0, 0.0, 0.0, 0.0], (4, 1));
        let field_b =
            OpticalRotorField::from_phase(vec![FRAC_PI_4, FRAC_PI_4, FRAC_PI_4, FRAC_PI_4], (4, 1));

        let dist = tropical.phase_distance(&field_a, &field_b);
        assert!(approx_eq(dist, 4.0 * FRAC_PI_4, 1e-5));

        let norm_dist = tropical.normalized_phase_distance(&field_a, &field_b);
        assert!(approx_eq(norm_dist, FRAC_PI_4, 1e-5));
    }

    #[test]
    fn test_tropical_sum() {
        let tropical = TropicalOpticalAlgebra::new((4, 1));

        let field_a = OpticalRotorField::from_phase(vec![0.0, PI, 0.0, PI], (4, 1));
        let field_b = OpticalRotorField::from_phase(vec![FRAC_PI_2, 0.0, FRAC_PI_4, 0.0], (4, 1));
        let field_c =
            OpticalRotorField::from_phase(vec![FRAC_PI_4, FRAC_PI_4, PI, FRAC_PI_4], (4, 1));

        let result = tropical.tropical_sum([&field_a, &field_b, &field_c].iter().copied());
        let result = result.unwrap();

        // Should have minimum |phase| at each point
        assert!(approx_eq(result.phase_at(0, 0), 0.0, 1e-5)); // min(0, π/2, π/4) = 0
        assert!(approx_eq(result.phase_at(1, 0), 0.0, 1e-5)); // min(π, 0, π/4) = 0
        assert!(approx_eq(result.phase_at(2, 0), 0.0, 1e-5)); // min(0, π/4, π) = 0
        assert!(approx_eq(result.phase_at(3, 0), 0.0, 1e-5)); // min(π, 0, π/4) = 0
    }

    #[test]
    fn test_soft_tropical_add() {
        let tropical = TropicalOpticalAlgebra::new((4, 1));

        let field_a = OpticalRotorField::from_phase(vec![0.0, 0.0, 0.0, 0.0], (4, 1));
        let field_b =
            OpticalRotorField::from_phase(vec![FRAC_PI_2, FRAC_PI_2, FRAC_PI_2, FRAC_PI_2], (4, 1));

        // High beta should approach hard tropical add
        let hard = tropical.soft_tropical_add(&field_a, &field_b, 100.0);
        assert!(approx_eq(hard.phase_at(0, 0), 0.0, 1e-3));

        // Low beta should give something closer to average
        let soft = tropical.soft_tropical_add(&field_a, &field_b, 0.1);
        let soft_phase = soft.phase_at(0, 0).abs();
        // Should be between 0 and π/4 (closer to mean)
        assert!(soft_phase < FRAC_PI_2);
        assert!(soft_phase > 0.1);
    }

    #[test]
    fn test_attractor_convergence() {
        let tropical = TropicalOpticalAlgebra::new((8, 8));

        // Create some attractor fields
        let attractors = vec![
            OpticalRotorField::uniform(0.0, 1.0, (8, 8)),
            OpticalRotorField::uniform(FRAC_PI_4, 1.0, (8, 8)),
        ];

        // Start from random state
        let initial = OpticalRotorField::random((8, 8), 42);

        let (final_state, iterations) =
            tropical.attractor_converge(&initial, &attractors, 10, 1e-6);

        // Should converge quickly to the zero-phase attractor (smallest |phase|)
        assert!(iterations <= 2);
        assert!(approx_eq(final_state.phase_at(0, 0), 0.0, 1e-5));
    }
}
