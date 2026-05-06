//! Core dual number types for automatic differentiation
//!
//! Dual numbers extend real numbers with an infinitesimal unit ε where ε² = 0.
//! This allows for exact computation of derivatives without numerical approximation
//! or computational graphs, making it ideal for forward-mode automatic differentiation.
//!
//! A dual number has the form: a + b·ε where:
//! - a is the real part (the function value)
//! - b is the dual part (the derivative)

use crate::{DualError, DualResult};
use core::fmt;
use core::ops::{Add, Div, Mul, Neg, Sub};
use num_traits::{Float, One, Zero};

/// Tie policy for branch-sensitive operations such as `max` and `min`.
///
/// These operations are not classically differentiable when both branches have
/// equal real values, so the policy determines which derivative information is
/// propagated in that equality case.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BranchPolicy {
    /// Preserve the left branch on ties.
    #[default]
    Left,
    /// Preserve the right branch on ties.
    Right,
    /// Preserve the shared real value and average the derivative or gradient.
    Average,
}

/// A dual number for automatic differentiation
///
/// Dual numbers enable exact derivative computation using the algebraic property ε² = 0.
/// For a function f(x), evaluating f(x + ε) automatically computes both f(x) and f'(x).
///
/// # Examples
///
/// ```
/// use amari_dual::DualNumber;
///
/// // Create a variable x = 3.0 (with derivative dx/dx = 1.0)
/// let x = DualNumber::variable(3.0);
///
/// // Compute f(x) = x² + 2x + 1
/// let result = x * x + DualNumber::constant(2.0) * x + DualNumber::constant(1.0);
///
/// // result.real = f(3) = 9 + 6 + 1 = 16
/// // result.dual = f'(3) = 2(3) + 2 = 8
/// assert_eq!(result.real, 16.0);
/// assert_eq!(result.dual, 8.0);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DualNumber<T: Float> {
    /// The real part (function value)
    pub real: T,
    /// The dual part (derivative)
    pub dual: T,
}

impl<T: Float> DualNumber<T> {
    /// Create a new dual number
    pub fn new(real: T, dual: T) -> Self {
        Self { real, dual }
    }

    /// Create a constant (derivative = 0)
    ///
    /// Constants have zero derivative since d/dx(c) = 0.
    pub fn constant(value: T) -> Self {
        Self {
            real: value,
            dual: T::zero(),
        }
    }

    /// Create a variable (derivative = 1)
    ///
    /// Variables have unit derivative since d/dx(x) = 1.
    pub fn variable(value: T) -> Self {
        Self {
            real: value,
            dual: T::one(),
        }
    }

    /// Get the value (real part)
    pub fn value(&self) -> T {
        self.real
    }

    /// Get the derivative (dual part)
    pub fn derivative(&self) -> T {
        self.dual
    }

    /// Exponential function: exp(a + b·ε) = exp(a) + b·exp(a)·ε
    ///
    /// Uses the chain rule: d/dx(e^f) = f'·e^f
    pub fn exp(self) -> Self {
        let exp_real = self.real.exp();
        Self {
            real: exp_real,
            dual: self.dual * exp_real,
        }
    }

    /// Natural logarithm: ln(a + b·ε) = ln(a) + (b/a)·ε
    ///
    /// Uses the chain rule: d/dx(ln f) = f'/f
    pub fn ln(self) -> Self {
        Self {
            real: self.real.ln(),
            dual: self.dual / self.real,
        }
    }

    /// Sine function: sin(a + b·ε) = sin(a) + b·cos(a)·ε
    ///
    /// Uses the chain rule: d/dx(sin f) = f'·cos(f)
    pub fn sin(self) -> Self {
        Self {
            real: self.real.sin(),
            dual: self.dual * self.real.cos(),
        }
    }

    /// Cosine function: cos(a + b·ε) = cos(a) - b·sin(a)·ε
    ///
    /// Uses the chain rule: d/dx(cos f) = -f'·sin(f)
    pub fn cos(self) -> Self {
        Self {
            real: self.real.cos(),
            dual: -self.dual * self.real.sin(),
        }
    }

    /// Tangent function: tan(a + b·ε) = tan(a) + b·sec²(a)·ε
    ///
    /// Uses the chain rule: d/dx(tan f) = f'·sec²(f) = f'/cos²(f)
    pub fn tan(self) -> Self {
        let tan_real = self.real.tan();
        let cos_real = self.real.cos();
        Self {
            real: tan_real,
            dual: self.dual / (cos_real * cos_real),
        }
    }

    /// Square root: sqrt(a + b·ε) = sqrt(a) + (b/(2·sqrt(a)))·ε
    ///
    /// Uses the chain rule: d/dx(√f) = f'/(2√f)
    pub fn sqrt(self) -> Self {
        let sqrt_real = self.real.sqrt();
        Self {
            real: sqrt_real,
            dual: self.dual / (T::from(2.0).unwrap() * sqrt_real),
        }
    }

    /// Power function: (a + b·ε)^n = a^n + n·b·a^(n-1)·ε
    ///
    /// Uses the power rule: d/dx(f^n) = n·f'·f^(n-1)
    pub fn powf(self, n: T) -> Self {
        let pow_real = self.real.powf(n);
        Self {
            real: pow_real,
            dual: n * self.dual * self.real.powf(n - T::one()),
        }
    }

    /// Integer power function: (a + b·ε)^n = a^n + n·b·a^(n-1)·ε
    ///
    /// Uses the power rule: d/dx(f^n) = n·f'·f^(n-1)
    pub fn powi(self, n: i32) -> Self {
        let n_float = T::from(n).unwrap();
        let pow_real = self.real.powi(n);
        Self {
            real: pow_real,
            dual: n_float * self.dual * self.real.powi(n - 1),
        }
    }

    /// Absolute value: |a + b·ε| = |a| + b·sign(a)·ε
    ///
    /// Derivative is sign(a), undefined at a=0
    pub fn abs(self) -> Self {
        let sign = if self.real >= T::zero() {
            T::one()
        } else {
            -T::one()
        };
        Self {
            real: self.real.abs(),
            dual: self.dual * sign,
        }
    }

    /// Hyperbolic sine: sinh(a + b·ε) = sinh(a) + b·cosh(a)·ε
    pub fn sinh(self) -> Self {
        Self {
            real: self.real.sinh(),
            dual: self.dual * self.real.cosh(),
        }
    }

    /// Hyperbolic cosine: cosh(a + b·ε) = cosh(a) + b·sinh(a)·ε
    pub fn cosh(self) -> Self {
        Self {
            real: self.real.cosh(),
            dual: self.dual * self.real.sinh(),
        }
    }

    /// Hyperbolic tangent: tanh(a + b·ε) = tanh(a) + b·sech²(a)·ε
    pub fn tanh(self) -> Self {
        let tanh_real = self.real.tanh();
        let cosh_real = self.real.cosh();
        Self {
            real: tanh_real,
            dual: self.dual / (cosh_real * cosh_real),
        }
    }

    /// Maximum of two dual numbers.
    ///
    /// This method is **left-biased on ties** for backward compatibility:
    /// if `self.real == other.real`, the left branch is preserved.
    /// Use [`DualNumber::max_by_policy`] when equality behavior should be made explicit.
    pub fn max(self, other: Self) -> Self {
        self.max_by_policy(other, BranchPolicy::Left)
    }

    /// Minimum of two dual numbers.
    ///
    /// This method is **left-biased on ties** for backward compatibility:
    /// if `self.real == other.real`, the left branch is preserved.
    /// Use [`DualNumber::min_by_policy`] when equality behavior should be made explicit.
    pub fn min(self, other: Self) -> Self {
        self.min_by_policy(other, BranchPolicy::Left)
    }

    /// Maximum of two dual numbers with an explicit tie policy.
    pub fn max_by_policy(self, other: Self, policy: BranchPolicy) -> Self {
        if self.real > other.real {
            self
        } else if self.real < other.real {
            other
        } else {
            match policy {
                BranchPolicy::Left => self,
                BranchPolicy::Right => other,
                BranchPolicy::Average => Self {
                    real: self.real,
                    dual: (self.dual + other.dual) / T::from(2.0).unwrap(),
                },
            }
        }
    }

    /// Minimum of two dual numbers with an explicit tie policy.
    pub fn min_by_policy(self, other: Self, policy: BranchPolicy) -> Self {
        if self.real < other.real {
            self
        } else if self.real > other.real {
            other
        } else {
            match policy {
                BranchPolicy::Left => self,
                BranchPolicy::Right => other,
                BranchPolicy::Average => Self {
                    real: self.real,
                    dual: (self.dual + other.dual) / T::from(2.0).unwrap(),
                },
            }
        }
    }

    /// Sigmoid (logistic) function: σ(x) = 1/(1 + e^(-x))
    ///
    /// Uses the chain rule: d/dx(σ(f)) = σ(f)·(1 - σ(f))·f'
    pub fn sigmoid(self) -> Self {
        let exp_neg = (-self.real).exp();
        let sigmoid_real = T::one() / (T::one() + exp_neg);
        let sigmoid_deriv = sigmoid_real * (T::one() - sigmoid_real);
        Self {
            real: sigmoid_real,
            dual: self.dual * sigmoid_deriv,
        }
    }

    /// Apply a function with its derivative
    ///
    /// This is useful for applying functions where you know both f(x) and f'(x).
    /// The chain rule is applied automatically.
    ///
    /// # Arguments
    /// * `f` - The function to apply
    /// * `df` - The derivative of the function
    pub fn apply_with_derivative<F, G>(self, f: F, df: G) -> Self
    where
        F: Fn(T) -> T,
        G: Fn(T) -> T,
    {
        Self {
            real: f(self.real),
            dual: self.dual * df(self.real),
        }
    }
}

// Arithmetic operations using dual number algebra
//
// Dual number arithmetic rules (where ε² = 0):
// (a + b·ε) + (c + d·ε) = (a + c) + (b + d)·ε
// (a + b·ε) - (c + d·ε) = (a - c) + (b - d)·ε
// (a + b·ε) * (c + d·ε) = a·c + (a·d + b·c)·ε
// (a + b·ε) / (c + d·ε) = a/c + (b·c - a·d)/c²·ε

impl<T: Float> Add for DualNumber<T> {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self {
            real: self.real + other.real,
            dual: self.dual + other.dual,
        }
    }
}

impl<T: Float> Sub for DualNumber<T> {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        Self {
            real: self.real - other.real,
            dual: self.dual - other.dual,
        }
    }
}

impl<T: Float> Mul for DualNumber<T> {
    type Output = Self;

    fn mul(self, other: Self) -> Self {
        Self {
            real: self.real * other.real,
            dual: self.real * other.dual + self.dual * other.real,
        }
    }
}

impl<T: Float> Div for DualNumber<T> {
    type Output = Self;

    fn div(self, other: Self) -> Self {
        let real = self.real / other.real;
        let dual = (self.dual * other.real - self.real * other.dual) / (other.real * other.real);
        Self { real, dual }
    }
}

impl<T: Float> Neg for DualNumber<T> {
    type Output = Self;

    fn neg(self) -> Self {
        Self {
            real: -self.real,
            dual: -self.dual,
        }
    }
}

impl<T: Float> Zero for DualNumber<T> {
    fn zero() -> Self {
        Self::constant(T::zero())
    }

    fn is_zero(&self) -> bool {
        self.real.is_zero() && self.dual.is_zero()
    }
}

impl<T: Float> One for DualNumber<T> {
    fn one() -> Self {
        Self::constant(T::one())
    }
}

impl<T: Float + fmt::Display> fmt::Display for DualNumber<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} + {}ε", self.real, self.dual)
    }
}

// Precision type aliases for dual numbers
/// Standard-precision dual number (f64)
pub type StandardDual = DualNumber<f64>;

/// Extended-precision dual number (uses extended precision float from amari-core)
#[cfg(feature = "high-precision")]
pub type ExtendedDual = DualNumber<crate::ExtendedFloat>;

/// Standard-precision multi-dual number (f64)
pub type StandardMultiDual = MultiDualNumber<f64>;

/// Extended-precision multi-dual number (uses extended precision float from amari-core)
#[cfg(feature = "high-precision")]
pub type ExtendedMultiDual = MultiDualNumber<crate::ExtendedFloat>;

/// Standard-precision fixed-size multi-dual number (f64)
pub type StandardStaticMultiDual<const N: usize> = StaticMultiDual<f64, N>;

/// Extended-precision fixed-size multi-dual number (uses extended precision float from amari-core)
#[cfg(feature = "high-precision")]
pub type ExtendedStaticMultiDual<const N: usize> = StaticMultiDual<crate::ExtendedFloat, N>;

/// Multi-variable dual number for computing gradients
///
/// A MultiDualNumber represents a scalar function of multiple variables,
/// storing the function value and partial derivatives with respect to each variable.
///
/// # Examples
///
/// ```
/// use amari_dual::MultiDualNumber;
///
/// // f(x, y) = x² + xy + y²
/// // ∂f/∂x = 2x + y
/// // ∂f/∂y = x + 2y
///
/// let x = 2.0;
/// let y = 3.0;
///
/// let value = x * x + x * y + y * y; // = 4 + 6 + 9 = 19
/// let gradient = vec![2.0 * x + y, x + 2.0 * y]; // = [7, 8]
///
/// let result = MultiDualNumber::new(value, gradient);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct MultiDualNumber<T: Float> {
    /// The function value
    pub value: T,
    /// The gradient (partial derivatives with respect to each variable)
    pub gradient: Vec<T>,
}

impl<T: Float> MultiDualNumber<T> {
    /// Create a new multi-dual number
    pub fn new(value: T, gradient: Vec<T>) -> Self {
        Self { value, gradient }
    }

    /// Create a constant (all partial derivatives = 0)
    pub fn constant(value: T, n_vars: usize) -> Self {
        Self {
            value,
            gradient: vec![T::zero(); n_vars],
        }
    }

    /// Create a variable (partial derivative = 1 for this variable, 0 for others)
    pub fn variable(value: T, var_index: usize, n_vars: usize) -> Self {
        let mut gradient = vec![T::zero(); n_vars];
        gradient[var_index] = T::one();
        Self { value, gradient }
    }

    /// Create a basis-seeded variable vector from raw values.
    ///
    /// This is a convenience helper for small optimization problems where the
    /// caller wants one active variable per coordinate.
    pub fn variables(values: &[T]) -> Vec<Self> {
        values
            .iter()
            .enumerate()
            .map(|(index, &value)| Self::variable(value, index, values.len()))
            .collect()
    }

    /// Get the number of variables
    pub fn n_vars(&self) -> usize {
        self.gradient.len()
    }

    /// Get the value
    pub fn get_value(&self) -> T {
        self.value
    }

    /// Get the gradient
    pub fn get_gradient(&self) -> &[T] {
        &self.gradient
    }

    /// Maximum of two multi-dual numbers with an explicit tie policy.
    pub fn max_by_policy(self, other: Self, policy: BranchPolicy) -> Self {
        assert_eq!(
            self.gradient.len(),
            other.gradient.len(),
            "Gradient dimension mismatch"
        );

        if self.value > other.value {
            self
        } else if self.value < other.value {
            other
        } else {
            match policy {
                BranchPolicy::Left => self,
                BranchPolicy::Right => other,
                BranchPolicy::Average => {
                    let gradient = self
                        .gradient
                        .iter()
                        .zip(&other.gradient)
                        .map(|(&left, &right)| (left + right) / T::from(2.0).unwrap())
                        .collect();
                    Self {
                        value: self.value,
                        gradient,
                    }
                }
            }
        }
    }

    /// Minimum of two multi-dual numbers with an explicit tie policy.
    pub fn min_by_policy(self, other: Self, policy: BranchPolicy) -> Self {
        assert_eq!(
            self.gradient.len(),
            other.gradient.len(),
            "Gradient dimension mismatch"
        );

        if self.value < other.value {
            self
        } else if self.value > other.value {
            other
        } else {
            match policy {
                BranchPolicy::Left => self,
                BranchPolicy::Right => other,
                BranchPolicy::Average => {
                    let gradient = self
                        .gradient
                        .iter()
                        .zip(&other.gradient)
                        .map(|(&left, &right)| (left + right) / T::from(2.0).unwrap())
                        .collect();
                    Self {
                        value: self.value,
                        gradient,
                    }
                }
            }
        }
    }
}

impl<T: Float> Add for MultiDualNumber<T> {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        assert_eq!(
            self.gradient.len(),
            other.gradient.len(),
            "Gradient dimension mismatch"
        );
        let gradient = self
            .gradient
            .iter()
            .zip(&other.gradient)
            .map(|(&a, &b)| a + b)
            .collect();
        Self {
            value: self.value + other.value,
            gradient,
        }
    }
}

impl<T: Float> Sub for MultiDualNumber<T> {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        assert_eq!(
            self.gradient.len(),
            other.gradient.len(),
            "Gradient dimension mismatch"
        );
        let gradient = self
            .gradient
            .iter()
            .zip(&other.gradient)
            .map(|(&a, &b)| a - b)
            .collect();
        Self {
            value: self.value - other.value,
            gradient,
        }
    }
}

impl<T: Float> Mul for MultiDualNumber<T> {
    type Output = Self;

    fn mul(self, other: Self) -> Self {
        assert_eq!(
            self.gradient.len(),
            other.gradient.len(),
            "Gradient dimension mismatch"
        );
        // Product rule: (fg)' = f'g + fg'
        let gradient = self
            .gradient
            .iter()
            .zip(&other.gradient)
            .map(|(&df, &dg)| df * other.value + self.value * dg)
            .collect();
        Self {
            value: self.value * other.value,
            gradient,
        }
    }
}

impl<T: Float> Div for MultiDualNumber<T> {
    type Output = Self;

    fn div(self, other: Self) -> Self {
        assert_eq!(
            self.gradient.len(),
            other.gradient.len(),
            "Gradient dimension mismatch"
        );
        // Quotient rule: (f/g)' = (f'g - fg')/g²
        let g_squared = other.value * other.value;
        let gradient = self
            .gradient
            .iter()
            .zip(&other.gradient)
            .map(|(&df, &dg)| (df * other.value - self.value * dg) / g_squared)
            .collect();
        Self {
            value: self.value / other.value,
            gradient,
        }
    }
}

impl<T: Float> Neg for MultiDualNumber<T> {
    type Output = Self;

    fn neg(self) -> Self {
        let gradient = self.gradient.iter().map(|&x| -x).collect();
        Self {
            value: -self.value,
            gradient,
        }
    }
}

/// Fixed-size multi-variable dual number for small optimization loops.
///
/// This type stores gradients inline in an array, avoiding heap allocation for
/// the common case of very small parameter sets.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StaticMultiDual<T: Float, const N: usize> {
    /// The function value.
    pub value: T,
    /// The fixed-size gradient array.
    pub gradient: [T; N],
}

impl<T: Float, const N: usize> StaticMultiDual<T, N> {
    /// Create a new fixed-size multi-dual number.
    pub fn new(value: T, gradient: [T; N]) -> Self {
        Self { value, gradient }
    }

    /// Create a constant with zero gradient.
    pub fn constant(value: T) -> Self {
        Self {
            value,
            gradient: [T::zero(); N],
        }
    }

    /// Create a basis-seeded variable.
    pub fn variable(value: T, var_index: usize) -> Self {
        assert!(var_index < N, "Variable index out of bounds");
        let mut gradient = [T::zero(); N];
        gradient[var_index] = T::one();
        Self { value, gradient }
    }

    /// Create one variable per coordinate from raw values.
    pub fn variables(values: [T; N]) -> [Self; N] {
        core::array::from_fn(|index| Self::variable(values[index], index))
    }

    /// Get the number of variables.
    pub const fn n_vars(&self) -> usize {
        N
    }

    /// Get the value.
    pub fn get_value(&self) -> T {
        self.value
    }

    /// Get the gradient.
    pub fn get_gradient(&self) -> &[T; N] {
        &self.gradient
    }

    /// Convert to the heap-backed [`MultiDualNumber`].
    pub fn to_multi_dual(self) -> MultiDualNumber<T> {
        MultiDualNumber::new(self.value, self.gradient.to_vec())
    }

    /// Convert from a heap-backed [`MultiDualNumber`], validating the dimension.
    pub fn try_from_multi_dual(value: MultiDualNumber<T>) -> DualResult<Self> {
        let actual = value.gradient.len();
        if actual != N {
            return Err(DualError::DimensionMismatch {
                expected: N,
                actual,
            });
        }

        let gradient =
            value
                .gradient
                .try_into()
                .map_err(|gradient: Vec<T>| DualError::DimensionMismatch {
                    expected: N,
                    actual: gradient.len(),
                })?;

        Ok(Self {
            value: value.value,
            gradient,
        })
    }

    /// Maximum of two fixed-size multi-dual numbers with an explicit tie policy.
    pub fn max_by_policy(self, other: Self, policy: BranchPolicy) -> Self {
        if self.value > other.value {
            self
        } else if self.value < other.value {
            other
        } else {
            match policy {
                BranchPolicy::Left => self,
                BranchPolicy::Right => other,
                BranchPolicy::Average => Self {
                    value: self.value,
                    gradient: core::array::from_fn(|index| {
                        (self.gradient[index] + other.gradient[index]) / T::from(2.0).unwrap()
                    }),
                },
            }
        }
    }

    /// Minimum of two fixed-size multi-dual numbers with an explicit tie policy.
    pub fn min_by_policy(self, other: Self, policy: BranchPolicy) -> Self {
        if self.value < other.value {
            self
        } else if self.value > other.value {
            other
        } else {
            match policy {
                BranchPolicy::Left => self,
                BranchPolicy::Right => other,
                BranchPolicy::Average => Self {
                    value: self.value,
                    gradient: core::array::from_fn(|index| {
                        (self.gradient[index] + other.gradient[index]) / T::from(2.0).unwrap()
                    }),
                },
            }
        }
    }
}

impl<T: Float, const N: usize> Add for StaticMultiDual<T, N> {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self {
            value: self.value + other.value,
            gradient: core::array::from_fn(|index| self.gradient[index] + other.gradient[index]),
        }
    }
}

impl<T: Float, const N: usize> Sub for StaticMultiDual<T, N> {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        Self {
            value: self.value - other.value,
            gradient: core::array::from_fn(|index| self.gradient[index] - other.gradient[index]),
        }
    }
}

#[allow(clippy::suspicious_arithmetic_impl)]
impl<T: Float, const N: usize> Mul for StaticMultiDual<T, N> {
    type Output = Self;

    fn mul(self, other: Self) -> Self {
        Self {
            value: self.value * other.value,
            gradient: core::array::from_fn(|index| {
                self.gradient[index] * other.value + self.value * other.gradient[index]
            }),
        }
    }
}

impl<T: Float, const N: usize> Div for StaticMultiDual<T, N> {
    type Output = Self;

    fn div(self, other: Self) -> Self {
        let denominator = other.value * other.value;
        Self {
            value: self.value / other.value,
            gradient: core::array::from_fn(|index| {
                (self.gradient[index] * other.value - self.value * other.gradient[index])
                    / denominator
            }),
        }
    }
}

impl<T: Float, const N: usize> Neg for StaticMultiDual<T, N> {
    type Output = Self;

    fn neg(self) -> Self {
        Self {
            value: -self.value,
            gradient: core::array::from_fn(|index| -self.gradient[index]),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn test_dual_number_creation() {
        let constant = DualNumber::constant(5.0);
        assert_eq!(constant.real, 5.0);
        assert_eq!(constant.dual, 0.0);

        let variable = DualNumber::variable(3.0);
        assert_eq!(variable.real, 3.0);
        assert_eq!(variable.dual, 1.0);
    }

    #[test]
    fn test_dual_number_arithmetic() {
        let x = DualNumber::variable(2.0);
        let c = DualNumber::constant(3.0);

        // f(x) = 3x
        let result = c * x;
        assert_eq!(result.real, 6.0); // f(2) = 6
        assert_eq!(result.dual, 3.0); // f'(2) = 3

        // f(x) = x²
        let result = x * x;
        assert_eq!(result.real, 4.0); // f(2) = 4
        assert_eq!(result.dual, 4.0); // f'(2) = 2x = 4
    }

    #[test]
    fn test_dual_number_division() {
        let x = DualNumber::variable(4.0);
        let c = DualNumber::constant(2.0);

        // f(x) = x/2
        let result = x / c;
        assert_eq!(result.real, 2.0); // f(4) = 2
        assert_eq!(result.dual, 0.5); // f'(4) = 1/2
    }

    #[test]
    fn test_dual_number_exp() {
        let x = DualNumber::variable(0.0);

        // f(x) = e^x
        let result = x.exp();
        assert_relative_eq!(result.real, 1.0, epsilon = 1e-10); // e^0 = 1
        assert_relative_eq!(result.dual, 1.0, epsilon = 1e-10); // d/dx(e^x) = e^x = 1
    }

    #[test]
    fn test_dual_number_ln() {
        let x = DualNumber::variable(1.0);

        // f(x) = ln(x)
        let result = x.ln();
        assert_relative_eq!(result.real, 0.0, epsilon = 1e-10); // ln(1) = 0
        assert_relative_eq!(result.dual, 1.0, epsilon = 1e-10); // d/dx(ln x) = 1/x = 1
    }

    #[test]
    fn test_dual_number_sin() {
        let x = DualNumber::variable(0.0);

        // f(x) = sin(x)
        let result = x.sin();
        assert_relative_eq!(result.real, 0.0, epsilon = 1e-10); // sin(0) = 0
        assert_relative_eq!(result.dual, 1.0, epsilon = 1e-10); // d/dx(sin x) = cos(x) = 1
    }

    #[test]
    fn test_dual_number_cos() {
        let x = DualNumber::variable(0.0);

        // f(x) = cos(x)
        let result = x.cos();
        assert_relative_eq!(result.real, 1.0, epsilon = 1e-10); // cos(0) = 1
        assert_relative_eq!(result.dual, 0.0, epsilon = 1e-10); // d/dx(cos x) = -sin(x) = 0
    }

    #[test]
    fn test_dual_number_sqrt() {
        let x = DualNumber::variable(4.0);

        // f(x) = √x
        let result = x.sqrt();
        assert_relative_eq!(result.real, 2.0, epsilon = 1e-10); // √4 = 2
        assert_relative_eq!(result.dual, 0.25, epsilon = 1e-10); // d/dx(√x) = 1/(2√x) = 1/4
    }

    #[test]
    fn test_multi_dual_number() {
        // f(x, y) = x + y
        let x = MultiDualNumber::variable(2.0, 0, 2);
        let y = MultiDualNumber::variable(3.0, 1, 2);

        let result = x + y;
        assert_eq!(result.value, 5.0);
        assert_eq!(result.gradient[0], 1.0); // ∂f/∂x = 1
        assert_eq!(result.gradient[1], 1.0); // ∂f/∂y = 1
    }

    #[test]
    fn test_multi_dual_number_product() {
        // f(x, y) = x * y
        let x = MultiDualNumber::variable(2.0, 0, 2);
        let y = MultiDualNumber::variable(3.0, 1, 2);

        let result = x * y;
        assert_eq!(result.value, 6.0);
        assert_eq!(result.gradient[0], 3.0); // ∂f/∂x = y = 3
        assert_eq!(result.gradient[1], 2.0); // ∂f/∂y = x = 2
    }

    #[test]
    fn test_chain_rule() {
        // f(x) = sin(x²)
        let x = DualNumber::variable(1.0);
        let x_squared = x * x;
        let result = x_squared.sin();

        // f(1) = sin(1)
        assert_relative_eq!(result.real, 1.0_f64.sin(), epsilon = 1e-10);
        // f'(1) = cos(1) * 2x = cos(1) * 2
        assert_relative_eq!(result.dual, 1.0_f64.cos() * 2.0, epsilon = 1e-10);
    }

    #[test]
    fn test_dual_branch_policies() {
        let left = DualNumber::new(1.0, 2.0);
        let right = DualNumber::new(1.0, 6.0);

        assert_eq!(left.max(right), left);
        assert_eq!(left.max_by_policy(right, BranchPolicy::Left), left);
        assert_eq!(left.max_by_policy(right, BranchPolicy::Right), right);
        assert_eq!(
            left.max_by_policy(right, BranchPolicy::Average),
            DualNumber::new(1.0, 4.0)
        );
    }

    #[test]
    fn test_multi_dual_seed_helpers() {
        let vars = MultiDualNumber::variables(&[2.0, 3.0, 5.0]);

        assert_eq!(vars.len(), 3);
        assert_eq!(vars[0].gradient, vec![1.0, 0.0, 0.0]);
        assert_eq!(vars[1].gradient, vec![0.0, 1.0, 0.0]);
        assert_eq!(vars[2].gradient, vec![0.0, 0.0, 1.0]);
    }

    #[test]
    fn test_multi_dual_branch_policies() {
        let left = MultiDualNumber::new(1.0, vec![1.0, 0.0]);
        let right = MultiDualNumber::new(1.0, vec![0.0, 1.0]);

        let averaged = left
            .clone()
            .max_by_policy(right.clone(), BranchPolicy::Average);

        assert_eq!(
            left.clone()
                .max_by_policy(right.clone(), BranchPolicy::Left),
            left
        );
        assert_eq!(
            left.clone()
                .max_by_policy(right.clone(), BranchPolicy::Right),
            right
        );
        assert_eq!(averaged.value, 1.0);
        assert_eq!(averaged.gradient, vec![0.5, 0.5]);
    }

    #[test]
    fn test_static_multi_dual_arithmetic() {
        let x = StaticMultiDual::<f64, 2>::variable(2.0, 0);
        let y = StaticMultiDual::<f64, 2>::variable(3.0, 1);
        let result = x * y + x;

        assert_eq!(result.value, 8.0);
        assert_eq!(result.gradient, [4.0, 2.0]);
    }

    #[test]
    fn test_static_multi_dual_conversion() {
        let static_value = StaticMultiDual::<f64, 3>::new(5.0, [1.0, 2.0, 3.0]);
        let dynamic = static_value.to_multi_dual();
        let recovered = StaticMultiDual::<f64, 3>::try_from_multi_dual(dynamic.clone()).unwrap();
        let mismatch = StaticMultiDual::<f64, 2>::try_from_multi_dual(dynamic);

        assert_eq!(recovered, static_value);
        assert_eq!(
            mismatch,
            Err(DualError::DimensionMismatch {
                expected: 2,
                actual: 3,
            })
        );
    }

    #[test]
    fn test_static_multi_dual_variables_helper() {
        let vars = StaticMultiDual::<f64, 3>::variables([2.0, 3.0, 5.0]);

        assert_eq!(vars[0].gradient, [1.0, 0.0, 0.0]);
        assert_eq!(vars[1].gradient, [0.0, 1.0, 0.0]);
        assert_eq!(vars[2].gradient, [0.0, 0.0, 1.0]);
    }
}
