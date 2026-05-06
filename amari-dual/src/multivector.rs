//! Dual number multivectors for automatic differentiation in geometric algebra

use crate::DualNumber;
use alloc::vec::Vec;
use amari_core::Multivector;
use num_traits::{Float, Zero};

/// Multivector with dual number coefficients for automatic differentiation
#[derive(Clone, Debug)]
pub struct DualMultivector<T: Float, const P: usize, const Q: usize, const R: usize> {
    coefficients: Vec<DualNumber<T>>,
}

impl<T: Float, const P: usize, const Q: usize, const R: usize> DualMultivector<T, P, Q, R> {
    const DIM: usize = P + Q + R;
    const BASIS_COUNT: usize = 1 << Self::DIM;

    /// Create zero dual multivector
    pub fn zero() -> Self {
        let mut coeffs = Vec::with_capacity(Self::BASIS_COUNT);
        for _ in 0..Self::BASIS_COUNT {
            coeffs.push(DualNumber::constant(T::zero()));
        }
        Self {
            coefficients: coeffs,
        }
    }

    /// Create from dual number coefficients
    pub fn from_dual_coefficients(coeffs: Vec<DualNumber<T>>) -> Self {
        assert_eq!(coeffs.len(), Self::BASIS_COUNT);
        Self {
            coefficients: coeffs,
        }
    }

    /// Create dual multivector where each coefficient is a variable
    pub fn new_variables(values: &[T]) -> Self {
        let mut coeffs = Vec::with_capacity(Self::BASIS_COUNT);
        for i in 0..Self::BASIS_COUNT {
            if i < values.len() {
                coeffs.push(DualNumber::variable(values[i]));
            } else {
                coeffs.push(DualNumber::constant(T::zero()));
            }
        }
        Self {
            coefficients: coeffs,
        }
    }

    /// Create dual multivector where each coefficient is a variable - alias
    pub fn new_variable(values: &[T]) -> Self {
        Self::new_variables(values)
    }

    /// Create constant dual multivector from Multivector
    pub fn constant_mv(mv: &Multivector<P, Q, R>) -> Self {
        let mut coeffs = Vec::with_capacity(Self::BASIS_COUNT);
        for i in 0..Self::BASIS_COUNT {
            coeffs.push(DualNumber::constant(
                T::from(mv.get(i)).unwrap_or(T::zero()),
            ));
        }
        Self {
            coefficients: coeffs,
        }
    }

    /// Create constant dual multivector from slice
    pub fn constant(values: &[T]) -> Self {
        let mut coeffs = Vec::with_capacity(Self::BASIS_COUNT);
        for i in 0..Self::BASIS_COUNT {
            if i < values.len() {
                coeffs.push(DualNumber::constant(values[i]));
            } else {
                coeffs.push(DualNumber::constant(T::zero()));
            }
        }
        Self {
            coefficients: coeffs,
        }
    }

    /// Get coefficient at index
    pub fn get(&self, index: usize) -> DualNumber<T> {
        self.coefficients
            .get(index)
            .copied()
            .unwrap_or(DualNumber::constant(T::zero()))
    }

    /// Set coefficient at index
    pub fn set(&mut self, index: usize, value: DualNumber<T>) {
        if index < self.coefficients.len() {
            self.coefficients[index] = value;
        }
    }

    /// Get the value part (without derivatives)
    pub fn value(&self) -> Multivector<P, Q, R> {
        let values: Vec<f64> = self
            .coefficients
            .iter()
            .map(|coeff| coeff.real.to_f64().unwrap_or(0.0))
            .collect();
        Multivector::from_coefficients(values)
    }

    /// Get the derivative part
    pub fn derivative(&self) -> Multivector<P, Q, R> {
        let derivatives: Vec<f64> = self
            .coefficients
            .iter()
            .map(|coeff| coeff.dual.to_f64().unwrap_or(0.0))
            .collect();
        Multivector::from_coefficients(derivatives)
    }

    /// Get the real part (value) as a Multivector
    pub fn real_part(&self) -> Multivector<P, Q, R> {
        self.value()
    }

    /// Compute magnitude of the real part
    pub fn magnitude(&self) -> T {
        T::from(self.value().magnitude()).unwrap()
    }

    /// Create from real multivector (alias for constant_mv)
    pub fn from_real_multivector(mv: Multivector<P, Q, R>) -> Self {
        Self::constant_mv(&mv)
    }

    /// Create scalar dual multivector from DualNumber
    pub fn scalar(value: DualNumber<T>) -> Self {
        let mut result = Self::zero();
        result.coefficients[0] = value;
        result
    }

    /// Create a basis vector (unit vector in given direction)
    pub fn basis_vector(index: usize) -> Result<Self, &'static str> {
        if index >= Self::DIM {
            return Err("Index out of bounds for basis vector");
        }

        let mut result = Self::zero();
        let basis_index = 1 << index; // 2^index for single basis blade
        if basis_index < Self::BASIS_COUNT {
            result.coefficients[basis_index] = DualNumber::constant(T::one());
            Ok(result)
        } else {
            Err("Invalid basis vector index")
        }
    }

    /// Get the grade of the multivector (highest grade component with non-zero coefficient)
    pub fn grade(&self) -> usize {
        let mut highest_grade = 0;
        for (i, coeff) in self.coefficients.iter().enumerate() {
            if coeff.real != T::zero() || coeff.dual != T::zero() {
                let grade = i.count_ones() as usize;
                if grade > highest_grade {
                    highest_grade = grade;
                }
            }
        }
        highest_grade
    }

    /// Geometric product with automatic differentiation
    pub fn geometric_product(&self, other: &Self) -> Self {
        // Use the same Cayley table structure as regular geometric product
        // but apply dual number multiplication rules

        let mut result = Self::zero();

        for i in 0..Self::BASIS_COUNT {
            for j in 0..Self::BASIS_COUNT {
                let index = i ^ j; // Same blade combination rule

                // Dual number multiplication with chain rule
                let product = self.coefficients[i] * other.coefficients[j];

                // Apply the appropriate sign from Cayley table
                let sign = self.compute_cayley_sign(i, j);
                let signed_product = if sign > 0.0 { product } else { -product };

                result.coefficients[index] = result.coefficients[index] + signed_product;
            }
        }

        result
    }

    /// Simplified Cayley table sign computation (should use proper table)
    fn compute_cayley_sign(&self, i: usize, j: usize) -> f64 {
        // Simplified - in practice would use the Cayley table from amari-core
        let mut swaps = 0;
        let mut b = j;

        while b != 0 {
            let lowest_b = b & (!b + 1);
            b ^= lowest_b;
            let mask = lowest_b - 1;
            let count = (i & mask).count_ones();
            swaps += count;
        }

        if swaps % 2 == 0 {
            1.0
        } else {
            -1.0
        }
    }

    /// Reverse with automatic differentiation
    pub fn reverse(&self) -> Self {
        let mut result = Self::zero();

        for i in 0..Self::BASIS_COUNT {
            let grade = i.count_ones() as usize;
            let sign = if grade == 0 || (grade * (grade - 1) / 2).is_multiple_of(2) {
                1.0
            } else {
                -1.0
            };

            if sign > 0.0 {
                result.coefficients[i] = self.coefficients[i];
            } else {
                result.coefficients[i] = -self.coefficients[i];
            }
        }

        result
    }

    /// Grade projection with automatic differentiation
    pub fn grade_projection(&self, grade: usize) -> Self {
        let mut result = Self::zero();

        for i in 0..Self::BASIS_COUNT {
            if i.count_ones() as usize == grade {
                result.coefficients[i] = self.coefficients[i];
            }
        }

        result
    }

    /// Dual number norm (with automatic differentiation)
    pub fn norm_squared(&self) -> DualNumber<T> {
        let reversed = self.reverse();
        let product = self.geometric_product(&reversed);
        product.coefficients[0] // Scalar part
    }

    /// Dual number norm
    pub fn norm(&self) -> DualNumber<T> {
        self.norm_squared().sqrt()
    }

    /// Normalize with automatic differentiation
    pub fn normalize(&self) -> Self {
        let norm = self.norm();
        let mut result = Self::zero();

        for i in 0..Self::BASIS_COUNT {
            result.coefficients[i] = self.coefficients[i] / norm;
        }

        result
    }

    /// Exponential map with automatic differentiation
    pub fn exp(&self) -> Self {
        // For bivectors, use closed form with dual numbers
        let grade2 = self.grade_projection(2);
        let remainder = self.clone() - grade2.clone();

        if remainder.norm().value() < T::from(1e-10).unwrap() {
            // Pure bivector case
            let b_squared = grade2.geometric_product(&grade2).coefficients[0];

            if b_squared.real > T::from(-1e-14).unwrap() {
                // Hyperbolic case
                let norm = b_squared.sqrt();
                let cosh_norm = norm.apply_with_derivative(|x| x.cosh(), |x| x.sinh());
                let sinh_norm = norm.apply_with_derivative(|x| x.sinh(), |x| x.cosh());

                let mut result = Self::zero();
                result.coefficients[0] = cosh_norm;

                if !norm.is_zero() {
                    let factor = sinh_norm / norm;
                    for i in 0..Self::BASIS_COUNT {
                        if i.count_ones() == 2 {
                            // Bivector components
                            result.coefficients[i] = grade2.coefficients[i] * factor;
                        }
                    }
                }

                result
            } else {
                // Circular case
                let norm = (-b_squared).sqrt();
                let cos_norm = norm.apply_with_derivative(|x| x.cos(), |x| -x.sin());
                let sin_norm = norm.apply_with_derivative(|x| x.sin(), |x| x.cos());

                let mut result = Self::zero();
                result.coefficients[0] = cos_norm;

                let factor = sin_norm / norm;
                for i in 0..Self::BASIS_COUNT {
                    if i.count_ones() == 2 {
                        result.coefficients[i] = grade2.coefficients[i] * factor;
                    }
                }

                result
            }
        } else {
            // General case - use series expansion
            self.exp_series()
        }
    }

    /// Series expansion for exponential
    fn exp_series(&self) -> Self {
        let mut result = Self::zero();
        result.coefficients[0] = DualNumber::constant(T::one());

        let mut term = result.clone();

        for n in 1..20 {
            term = term.geometric_product(self);
            let factorial = T::from((1..=n).product::<usize>()).unwrap();
            let scaled_term = term.clone() * DualNumber::constant(T::one() / factorial);

            result = result + scaled_term;

            // Check convergence (simplified)
            if term.norm().value() < T::from(1e-14).unwrap() {
                break;
            }
        }

        result
    }

    /// Apply a function element-wise with automatic differentiation
    pub fn map<F, G>(&self, f: F, df: G) -> Self
    where
        F: Fn(T) -> T,
        G: Fn(T) -> T,
    {
        let mut result = Self::zero();
        for i in 0..Self::BASIS_COUNT {
            result.coefficients[i] = self.coefficients[i].apply_with_derivative(&f, &df);
        }
        result
    }

    /// Forward-mode automatic differentiation
    ///
    /// Computes both the value and gradient of a function using dual numbers.
    /// Forward-mode AD propagates derivatives alongside function evaluation,
    /// allowing exact derivative computation without finite differences.
    ///
    /// # Parameters
    /// - `f`: A function that takes a `DualMultivector` and returns a `DualMultivector`
    ///
    /// # Returns
    /// A tuple containing:
    /// - The scalar value of the function at the input point
    /// - The full `DualMultivector` result containing all partial derivatives
    ///
    /// # Example
    /// ```
    /// use amari_dual::{DualMultivector, DualNumber};
    /// let input = DualMultivector::<f64, 3, 0, 0>::scalar(DualNumber::constant(2.0));
    /// let (value, gradient) = input.forward_mode_ad(|x| {
    ///     x.geometric_product(&x)  // x^2 in geometric algebra
    /// });
    /// // Computes geometric square and its derivative
    /// ```
    ///
    /// # How it works
    /// The dual number coefficients automatically track derivatives through
    /// all arithmetic operations via operator overloading. The 'dual' part
    /// of each number carries the derivative information, which is propagated
    /// according to the chain rule during computation.
    pub fn forward_mode_ad<F>(&self, f: F) -> (T, Self)
    where
        F: Fn(Self) -> Self,
    {
        let result = f(self.clone());
        let value = result.coefficients[0].real; // Extract scalar part value
        let gradient = result; // The result contains gradients
        (value, gradient)
    }
}

// Arithmetic operations
impl<T: Float, const P: usize, const Q: usize, const R: usize> core::ops::Add
    for DualMultivector<T, P, Q, R>
{
    type Output = Self;

    fn add(mut self, other: Self) -> Self {
        for i in 0..Self::BASIS_COUNT {
            self.coefficients[i] = self.coefficients[i] + other.coefficients[i];
        }
        self
    }
}

impl<T: Float, const P: usize, const Q: usize, const R: usize> core::ops::Sub
    for DualMultivector<T, P, Q, R>
{
    type Output = Self;

    fn sub(mut self, other: Self) -> Self {
        for i in 0..Self::BASIS_COUNT {
            self.coefficients[i] = self.coefficients[i] - other.coefficients[i];
        }
        self
    }
}

impl<T: Float, const P: usize, const Q: usize, const R: usize> core::ops::Mul<DualNumber<T>>
    for DualMultivector<T, P, Q, R>
{
    type Output = Self;

    fn mul(mut self, scalar: DualNumber<T>) -> Self {
        for i in 0..Self::BASIS_COUNT {
            self.coefficients[i] = self.coefficients[i] * scalar;
        }
        self
    }
}

/// Multi-variable dual multivector for computing full Jacobians
#[derive(Clone, Debug)]
pub struct MultiDualMultivector<T: Float> {
    /// Function values for each basis component
    pub values: Vec<T>,
    /// Jacobian matrix: `jacobian[basis_component][variable]`
    pub jacobian: Vec<Vec<T>>,
    pub n_vars: usize,
    pub basis_count: usize,
}

impl<T: Float> MultiDualMultivector<T> {
    /// Create new multi-dual multivector
    pub fn new(values: Vec<T>, n_vars: usize) -> Self {
        let basis_count = values.len();
        let mut jacobian = Vec::with_capacity(basis_count);
        for _ in 0..basis_count {
            let mut row = Vec::with_capacity(n_vars);
            for _ in 0..n_vars {
                row.push(T::zero());
            }
            jacobian.push(row);
        }

        Self {
            values,
            jacobian,
            n_vars,
            basis_count,
        }
    }

    /// Create variable multivector (one variable per coefficient)
    pub fn variables(values: Vec<T>) -> Self {
        let n_vars = values.len();
        let basis_count = values.len();
        let mut jacobian = Vec::with_capacity(basis_count);
        for _ in 0..basis_count {
            let mut row = Vec::with_capacity(n_vars);
            for _ in 0..n_vars {
                row.push(T::zero());
            }
            jacobian.push(row);
        }

        // Set up identity jacobian
        for (i, row) in jacobian.iter_mut().enumerate().take(basis_count) {
            if i < n_vars {
                row[i] = T::one();
            }
        }

        Self {
            values,
            jacobian,
            n_vars,
            basis_count,
        }
    }

    /// Get partial derivative of coefficient i with respect to variable j
    pub fn partial(&self, coeff_index: usize, var_index: usize) -> T {
        self.jacobian
            .get(coeff_index)
            .and_then(|row| row.get(var_index))
            .copied()
            .unwrap_or(T::zero())
    }

    /// Get full gradient of coefficient i
    pub fn gradient(&self, coeff_index: usize) -> Vec<T> {
        self.jacobian.get(coeff_index).cloned().unwrap_or_else(|| {
            let mut grad = Vec::with_capacity(self.n_vars);
            for _ in 0..self.n_vars {
                grad.push(T::zero());
            }
            grad
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use approx::assert_relative_eq;

    #[test]
    fn test_dual_multivector_creation() {
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        let dmv = DualMultivector::<f64, 3, 0, 0>::new_variables(&values);

        assert_eq!(dmv.coefficients.len(), 8);
        assert_eq!(dmv.get(0).real, 1.0);
        assert_eq!(dmv.get(0).dual, 1.0); // Variable has derivative 1
    }

    #[test]
    fn test_dual_geometric_product() {
        let values1 = vec![1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let values2 = vec![0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];

        let dmv1 = DualMultivector::<f64, 3, 0, 0>::new_variables(&values1);
        let dmv2 = DualMultivector::<f64, 3, 0, 0>::new_variables(&values2);

        let product = dmv1.geometric_product(&dmv2);

        // Should have non-zero derivative
        assert!(!product.get(1).is_zero());
    }

    #[test]
    fn test_dual_norm() {
        let values = vec![3.0, 4.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let dmv = DualMultivector::<f64, 3, 0, 0>::new_variables(&values);

        let norm = dmv.norm();

        // Norm should be 5.0
        assert_relative_eq!(norm.real, 5.0, epsilon = 1e-10);

        // Derivative should be non-zero
        assert!(norm.dual.abs() > 1e-10);
    }

    #[test]
    fn test_dual_exp() {
        // Create a small bivector
        let mut values = vec![0.0; 8];
        values[3] = 0.1; // Small bivector component

        let dmv = DualMultivector::<f64, 3, 0, 0>::new_variables(&values);
        let exp_result = dmv.exp();

        // Should have computed exponential
        assert!(exp_result.get(0).real > 0.9); // Close to 1 for small bivector
        assert!(exp_result.get(0).dual.abs() > 0.0); // Should have derivative
    }

    #[test]
    fn test_multi_dual_multivector() {
        let values = vec![1.0, 2.0, 3.0, 4.0];
        let mdmv = MultiDualMultivector::variables(values.clone());

        assert_eq!(mdmv.values, values);
        assert_eq!(mdmv.n_vars, 4);

        // Check identity jacobian
        for i in 0..4 {
            assert_eq!(mdmv.partial(i, i), 1.0);
            for j in 0..4 {
                if i != j {
                    assert_eq!(mdmv.partial(i, j), 0.0);
                }
            }
        }
    }
}
