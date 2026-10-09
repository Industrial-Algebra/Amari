//! Rotor operations for rotations and reflections

use crate::{Bivector, Multivector, Vector};
use alloc::vec::Vec;

/// Rotor in a Clifford algebra (even-grade multivector with unit norm)
pub struct Rotor<const P: usize, const Q: usize, const R: usize> {
    multivector: Multivector<P, Q, R>,
}

impl<const P: usize, const Q: usize, const R: usize> Rotor<P, Q, R> {
    /// Create a rotor from a bivector using exponential map
    ///
    /// For a bivector B representing a plane and angle θ,
    /// the rotor R = exp(-B*θ/2) performs a right-handed rotation by angle θ in that plane.
    ///
    /// The negative sign ensures the rotation follows the right-hand rule convention:
    /// when the thumb points in the direction of the bivector orientation,
    /// the fingers curl in the direction of positive rotation.
    pub fn from_bivector(bivector: &Bivector<P, Q, R>, angle: f64) -> Self {
        let half_angle_bivector = &bivector.mv * (-angle / 2.0); // Negative for right-handed rotation
        let rotor = half_angle_bivector.exp();

        Self {
            multivector: rotor.normalize().unwrap_or(Multivector::scalar(1.0)),
        }
    }

    /// Create a rotor from a raw multivector bivector
    ///
    /// Similar to `from_bivector` but accepts a raw `Multivector` representing a bivector.
    /// Uses the same right-handed rotation convention.
    pub fn from_multivector_bivector(bivector: &Multivector<P, Q, R>, angle: f64) -> Self {
        let half_angle_bivector = bivector * (-angle / 2.0); // Negative for right-handed rotation
        let rotor = half_angle_bivector.exp();

        Self {
            multivector: rotor.normalize().unwrap_or(Multivector::scalar(1.0)),
        }
    }

    /// Apply rotor to transform a multivector: R * v * R†
    pub fn apply(&self, v: &Multivector<P, Q, R>) -> Multivector<P, Q, R> {
        let r_dagger = self.multivector.reverse();
        self.multivector
            .geometric_product(v)
            .geometric_product(&r_dagger)
    }

    /// Get the underlying multivector
    pub fn as_multivector(&self) -> &Multivector<P, Q, R> {
        &self.multivector
    }

    /// Compose two rotors (multiply them)
    pub fn compose(&self, other: &Self) -> Self {
        let composed = self.multivector.geometric_product(&other.multivector);
        Self {
            multivector: composed.normalize().unwrap_or(Multivector::scalar(1.0)),
        }
    }

    /// Get the inverse rotor (reverses the rotation)
    pub fn inverse(&self) -> Self {
        Self {
            multivector: self.multivector.reverse(),
        }
    }

    /// Create identity rotor
    pub fn identity() -> Self {
        Self {
            multivector: Multivector::scalar(1.0),
        }
    }

    /// Get the magnitude of the rotor
    pub fn magnitude(&self) -> f64 {
        self.multivector.norm()
    }

    /// Get scalar part of the rotor
    pub fn scalar_part(&self) -> f64 {
        self.multivector.scalar_part()
    }

    /// Get bivector part of the rotor
    pub fn bivector_part(&self) -> Bivector<P, Q, R> {
        self.multivector.bivector_type()
    }

    /// Get coefficients as slice
    pub fn as_slice(&self) -> &[f64] {
        self.multivector.as_slice()
    }

    /// Apply rotor to a Vector
    pub fn apply_to_vector(&self, v: &Vector<P, Q, R>) -> Vector<P, Q, R> {
        let result = self.apply(&v.mv);
        Vector::from_multivector(&result)
    }

    /// Geometric product with another rotor
    pub fn geometric_product(&self, other: &Self) -> Self {
        self.compose(other)
    }

    /// Create rotor from vectors (typed version)
    pub fn from_vectors(a: &Vector<P, Q, R>, b: &Vector<P, Q, R>) -> Option<Self> {
        Self::from_vectors_mv(&a.mv, &b.mv)
    }

    /// Create rotor from vectors (multivector version)
    pub fn from_vectors_mv(a: &Multivector<P, Q, R>, b: &Multivector<P, Q, R>) -> Option<Self> {
        let a_norm = a.normalize()?;
        let b_norm = b.normalize()?;

        // R = (1 + b*a) / |1 + b*a|
        let ba = b_norm.geometric_product(&a_norm);
        let rotor_unnorm = Multivector::scalar(1.0) + ba;
        let rotor = rotor_unnorm.normalize()?;

        Some(Self { multivector: rotor })
    }

    /// Create rotor from axis-angle representation
    pub fn from_axis_angle(axis: &Vector<P, Q, R>, angle: f64) -> Self {
        // Convert axis to a bivector perpendicular to it
        // For 3D, the bivector is the dual of the axis vector
        let normalized_axis = axis.mv.normalize().unwrap_or(axis.mv.clone());

        // Create bivector from the axis vector using the dual operation
        // For a 3D axis (a1, a2, a3), the corresponding bivector is (a3*e12 - a2*e13 + a1*e23)
        let a1 = normalized_axis.get(1); // e1 component
        let a2 = normalized_axis.get(2); // e2 component
        let a3 = normalized_axis.get(4); // e3 component (corrected indexing)

        let mut bivector = Multivector::zero();
        bivector.set_bivector_component(0, a3); // e12 component
        bivector.set_bivector_component(1, -a2); // e13 component
        bivector.set_bivector_component(2, a1); // e23 component

        Self::from_multivector_bivector(&bivector, angle)
    }

    /// Spherical linear interpolation between rotors
    pub fn slerp(&self, other: &Self, t: f64) -> Self {
        // Proper SLERP implementation for rotors (quaternion-like)
        // Compute the inner product (dot product) between rotors
        let mut dot = 0.0;
        for i in 0..8 {
            dot += self.as_slice()[i] * other.as_slice()[i];
        }

        // Clamp dot product to avoid numerical issues
        let dot = dot.clamp(-1.0, 1.0);

        // Determine shortest path and handle quaternion double cover
        let (dot, other_sign) = if dot < 0.0 { (-dot, -1.0) } else { (dot, 1.0) };

        // If rotors are nearly identical, use linear interpolation
        const EPSILON: f64 = 0.9995;
        if dot > EPSILON {
            // Linear interpolation for small angles
            let mut result_coeffs = [0.0; 8];
            for (i, coeff) in result_coeffs.iter_mut().enumerate() {
                *coeff = self.as_slice()[i]
                    + t * (other_sign * other.as_slice()[i] - self.as_slice()[i]);
            }

            let mut result_mv = Multivector::zero();
            for (i, &coeff) in result_coeffs.iter().enumerate() {
                result_mv.set(i, coeff);
            }
            let normalized = result_mv.normalize().unwrap_or(result_mv);
            return Self {
                multivector: normalized,
            };
        }

        // Calculate angle between rotors
        let theta = dot.acos();
        let theta_t = theta * t;
        let sin_theta = theta.sin();
        let sin_theta_t = theta_t.sin();
        let sin_theta_1_t = (theta * (1.0 - t)).sin();

        // Spherical interpolation formula
        let scale0 = sin_theta_1_t / sin_theta;
        let scale1 = sin_theta_t / sin_theta;

        let mut result_coeffs = [0.0; 8];
        for (i, coeff) in result_coeffs.iter_mut().enumerate() {
            *coeff = scale0 * self.as_slice()[i] + scale1 * other_sign * other.as_slice()[i];
        }

        let mut result_mv = Multivector::zero();
        for (i, &coeff) in result_coeffs.iter().enumerate() {
            result_mv.set(i, coeff);
        }

        // Normalize to ensure it's a unit rotor
        let normalized = result_mv.normalize().unwrap_or(result_mv);

        Self {
            multivector: normalized,
        }
    }

    /// Convert rotor to rotation matrix (3x3 for 3D)
    ///
    /// Computes the matrix M such that M*v = R*v*R† for column vectors.
    /// Each column j is the image of basis vector e_j under the rotation.
    pub fn to_rotation_matrix(&self) -> [[f64; 3]; 3] {
        // Compute rotated basis vectors
        let cols: Vec<_> = (0..3)
            .map(|j| {
                let basis = Multivector::basis_vector(j);
                self.apply(&basis)
            })
            .collect();

        // Build matrix: M[i][j] = (rotated e_j).component(i)
        let mut matrix = [[0.0; 3]; 3];
        for (i, row) in matrix.iter_mut().enumerate() {
            for (j, entry) in row.iter_mut().enumerate() {
                *entry = cols[j].vector_component(i);
            }
        }

        matrix
    }

    /// Compute logarithm of rotor
    ///
    /// For a unit rotor R = cos(θ/2) + sin(θ/2)B̂, returns (θ/2)B̂
    /// where B̂ is the unit bivector of the rotation plane.
    pub fn logarithm(&self) -> Multivector<P, Q, R> {
        let w = self.scalar_part().clamp(-1.0, 1.0);
        let half_angle = w.acos(); // θ/2

        let bivector_part = self.multivector.grade_projection(2);
        let biv_norm = bivector_part.norm();

        if biv_norm < 1e-14 {
            // Near-identity rotor or π rotation with no bivector component
            // Return zero bivector (identity has zero log)
            return Multivector::zero();
        }

        // Normalize bivector to get B̂, then scale by half_angle
        // log(R) = (θ/2) * B̂ = (θ/2) * bivector_part / |bivector_part|
        bivector_part * (half_angle / biv_norm)
    }

    /// Raise rotor to a power
    ///
    /// Computes R^t = exp(t * log(R)) for any real exponent t.
    /// For a rotor representing rotation by angle θ, R^t rotates by tθ.
    pub fn power(&self, exponent: f64) -> Self {
        if exponent.abs() < 1e-14 {
            return Self::identity();
        }

        let log_r = self.logarithm();

        // If log is zero (identity rotor), any power is still identity
        if log_r.norm() < 1e-14 {
            return Self::identity();
        }

        let scaled = log_r * exponent;
        let result = scaled.exp();

        Self {
            multivector: result.normalize().unwrap_or(Multivector::scalar(1.0)),
        }
    }
}

/// Reflection through a hyperplane defined by a unit vector
pub fn reflect<const P: usize, const Q: usize, const R: usize>(
    v: &Multivector<P, Q, R>,
    n: &Multivector<P, Q, R>,
) -> Multivector<P, Q, R> {
    // Reflection formula: n * v * n (for unit normal vector)
    n.geometric_product(v).geometric_product(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rotor_90_degrees() {
        let e1 = Vector::<3, 0, 0>::e1();
        let _e2 = Vector::<3, 0, 0>::e2();
        let e12 = Bivector::<3, 0, 0>::e12();

        // Create 90-degree rotation in e1-e2 plane
        let rotor = Rotor::from_bivector(&e12, core::f64::consts::PI / 2.0);

        // Apply to e1, should get e2
        let rotated = rotor.apply_to_vector(&e1);
        assert!((rotated.mv.vector_component(1) - 1.0).abs() < 1e-10); // Should be e2
        assert!(rotated.mv.vector_component(0).abs() < 1e-10); // e1 component should be ~0
    }

    #[test]
    fn test_rotor_composition() {
        let e12 = Bivector::<3, 0, 0>::e12();

        // Two 45-degree rotations
        let rotor1 = Rotor::from_bivector(&e12, core::f64::consts::PI / 4.0);
        let rotor2 = Rotor::from_bivector(&e12, core::f64::consts::PI / 4.0);

        // Compose them
        let composed = rotor1.compose(&rotor2);

        // Should equal a single 90-degree rotation
        let rotor90 = Rotor::from_bivector(&e12, core::f64::consts::PI / 2.0);

        let diff = composed.as_multivector() - rotor90.as_multivector();
        assert!(diff.norm() < 1e-10);
    }

    #[test]
    fn test_reflection() {
        let e1 = Vector::<3, 0, 0>::e1();
        let mut v = Multivector::<3, 0, 0>::zero();
        v.set_vector_component(0, 1.0); // e1
        v.set_vector_component(1, 1.0); // e2

        // Reflect across e1 (should negate e2 component)
        let reflected = reflect(&v, &e1.mv);
        assert!((reflected.vector_component(0) - 1.0).abs() < 1e-10); // e1 unchanged
        assert!((reflected.vector_component(1) + 1.0).abs() < 1e-10); // e2 negated
    }
}
