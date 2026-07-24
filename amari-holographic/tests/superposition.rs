// SPDX-License-Identifier: MIT OR Apache-2.0

//! Additive superposition semantics for `BindingAlgebra`.

use amari_holographic::{
    AlgebraError, AlgebraResult, BindingAlgebra, CliffordAlgebra, FHRRAlgebra, MAPAlgebra,
};

#[derive(Clone, Debug)]
struct TestAlgebra(Vec<f64>);

impl BindingAlgebra for TestAlgebra {
    fn dimension(&self) -> usize {
        self.0.len()
    }

    fn identity() -> Self {
        Self(vec![1.0; 4])
    }

    fn zero() -> Self {
        Self(vec![0.0; 4])
    }

    fn bind(&self, other: &Self) -> Self {
        Self(
            self.0
                .iter()
                .zip(&other.0)
                .map(|(left, right)| left * right)
                .collect(),
        )
    }

    fn inverse(&self) -> AlgebraResult<Self> {
        if self.0.iter().any(|value| value.abs() <= f64::EPSILON) {
            return Err(AlgebraError::NotInvertible {
                reason: "zero test coefficient".to_string(),
            });
        }
        Ok(Self(self.0.iter().map(|value| value.recip()).collect()))
    }

    fn bundle(&self, other: &Self, _beta: f64) -> AlgebraResult<Self> {
        if self.dimension() != other.dimension() {
            return Err(AlgebraError::DimensionMismatch {
                expected: self.dimension(),
                actual: other.dimension(),
            });
        }
        Ok(Self(
            self.0
                .iter()
                .zip(&other.0)
                .map(|(left, right)| (left + right) / 2.0)
                .collect(),
        ))
    }

    fn similarity(&self, other: &Self) -> f64 {
        let denominator = self.norm() * other.norm();
        if denominator <= f64::EPSILON {
            return 0.0;
        }
        self.0
            .iter()
            .zip(&other.0)
            .map(|(left, right)| left * right)
            .sum::<f64>()
            / denominator
    }

    fn norm(&self) -> f64 {
        self.0.iter().map(|value| value * value).sum::<f64>().sqrt()
    }

    fn normalize(&self) -> AlgebraResult<Self> {
        let norm = self.norm();
        if norm <= f64::EPSILON {
            return Err(AlgebraError::NormalizationFailed { norm });
        }
        Ok(Self(self.0.iter().map(|value| value / norm).collect()))
    }

    fn permute(&self, _shift: i32) -> Self {
        self.clone()
    }

    fn get(&self, index: usize) -> AlgebraResult<f64> {
        self.0
            .get(index)
            .copied()
            .ok_or(AlgebraError::IndexOutOfBounds {
                index,
                size: self.dimension(),
            })
    }

    fn set(&mut self, index: usize, value: f64) -> AlgebraResult<()> {
        let size = self.dimension();
        let coefficient = self
            .0
            .get_mut(index)
            .ok_or(AlgebraError::IndexOutOfBounds { index, size })?;
        *coefficient = value;
        Ok(())
    }

    fn from_coefficients(coeffs: &[f64]) -> AlgebraResult<Self> {
        Ok(Self(coeffs.to_vec()))
    }

    fn to_coefficients(&self) -> Vec<f64> {
        self.0.clone()
    }

    fn algebra_name() -> &'static str {
        "test-algebra"
    }
}

fn assert_coefficients_close(actual: &[f64], expected: &[f64]) {
    assert_eq!(actual.len(), expected.len());
    for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
        assert!(
            (actual - expected).abs() <= 1.0e-12,
            "coefficient {index}: expected {expected}, got {actual}"
        );
    }
}

#[test]
fn default_superpose_is_coefficient_wise_addition() {
    let left = TestAlgebra(vec![1.0, -2.0, 3.5, 0.25]);
    let right = TestAlgebra(vec![4.0, 2.5, -1.5, 0.75]);

    let sum = left.superpose(&right).unwrap();

    assert_coefficients_close(&sum.to_coefficients(), &[5.0, 0.5, 2.0, 1.0]);
}

#[test]
fn default_scale_is_coefficient_wise_scalar_multiplication() {
    let value = TestAlgebra(vec![1.0, -2.0, 3.5, 0.25]);

    let scaled = value.scale(-2.0).unwrap();

    assert_coefficients_close(&scaled.to_coefficients(), &[-2.0, 4.0, -7.0, -0.5]);
}

#[test]
fn superposition_is_commutative_and_has_zero_identity() {
    let left = TestAlgebra(vec![1.0, -2.0, 3.0, -4.0]);
    let right = TestAlgebra(vec![-4.0, 3.0, -2.0, 1.0]);

    let left_right = left.superpose(&right).unwrap();
    let right_left = right.superpose(&left).unwrap();
    let with_zero = left.superpose(&TestAlgebra::zero()).unwrap();

    assert_coefficients_close(&left_right.to_coefficients(), &right_left.to_coefficients());
    assert_coefficients_close(&with_zero.to_coefficients(), &left.to_coefficients());
}

#[test]
fn dimension_mismatch_is_typed() {
    let left = TestAlgebra(vec![1.0, 2.0]);
    let right = TestAlgebra(vec![1.0, 2.0, 3.0]);

    let error = left.superpose(&right).unwrap_err();

    assert_eq!(
        error,
        AlgebraError::DimensionMismatch {
            expected: 2,
            actual: 3,
        }
    );
}

#[test]
fn repeated_superposition_grows_while_bundle_remains_attention_like() {
    let basis = [
        TestAlgebra(vec![1.0, 0.0, 0.0, 0.0]),
        TestAlgebra(vec![0.0, 1.0, 0.0, 0.0]),
        TestAlgebra(vec![0.0, 0.0, 1.0, 0.0]),
        TestAlgebra(vec![0.0, 0.0, 0.0, 1.0]),
    ];
    let mut trace = TestAlgebra::zero();
    for item in &basis {
        trace = trace.superpose(item).unwrap();
    }
    let mut attention = basis[0].clone();
    for item in basis.iter().skip(1) {
        attention = attention.bundle(item, 1.0).unwrap();
    }

    assert!((trace.norm() - 2.0).abs() <= 1.0e-12);
    assert!(attention.norm() <= 1.0);
    assert!(trace.norm() > attention.norm());
}

#[test]
fn map_defaults_match_explicit_coefficient_reconstruction() {
    type Map = MAPAlgebra<4>;
    let left = Map::from_coefficients(&[1.0, -2.0, 0.5, 4.0]).unwrap();
    let right = Map::from_coefficients(&[-3.0, 1.0, 2.5, -1.0]).unwrap();
    let expected_sum = Map::from_coefficients(&[-2.0, -1.0, 3.0, 3.0]).unwrap();
    let expected_scaled = Map::from_coefficients(&[0.25, -0.5, 0.125, 1.0]).unwrap();

    let sum = <Map as BindingAlgebra>::superpose(&left, &right).unwrap();
    let scaled = <Map as BindingAlgebra>::scale(&left, 0.25).unwrap();

    assert_coefficients_close(&sum.to_coefficients(), &expected_sum.to_coefficients());
    assert_coefficients_close(
        &scaled.to_coefficients(),
        &expected_scaled.to_coefficients(),
    );
}

#[test]
fn fhrr_superposition_preserves_complex_components() {
    type Fhrr = FHRRAlgebra<2>;
    let left = Fhrr::new([1.0, -2.0], [3.0, 4.0]);
    let right = Fhrr::new([0.5, 2.5], [-1.0, 0.25]);

    let sum = <Fhrr as BindingAlgebra>::superpose(&left, &right).unwrap();
    let scaled = <Fhrr as BindingAlgebra>::scale(&left, -2.0).unwrap();

    assert_coefficients_close(&[sum.real(0).unwrap(), sum.real(1).unwrap()], &[1.5, 0.5]);
    assert_coefficients_close(&[sum.imag(0).unwrap(), sum.imag(1).unwrap()], &[2.0, 4.25]);
    assert_coefficients_close(
        &[scaled.real(0).unwrap(), scaled.real(1).unwrap()],
        &[-2.0, 4.0],
    );
    assert_coefficients_close(
        &[scaled.imag(0).unwrap(), scaled.imag(1).unwrap()],
        &[-6.0, -8.0],
    );
}

#[test]
fn clifford_defaults_match_explicit_coefficient_reconstruction() {
    type Cl2 = CliffordAlgebra<2, 0, 0>;
    let left = Cl2::from_coefficients(&[1.0, 2.0, -1.0, 0.5]).unwrap();
    let right = Cl2::from_coefficients(&[-0.5, 1.0, 3.0, -2.0]).unwrap();
    let expected_sum = Cl2::from_coefficients(&[0.5, 3.0, 2.0, -1.5]).unwrap();
    let expected_scaled = Cl2::from_coefficients(&[-2.0, -4.0, 2.0, -1.0]).unwrap();

    let sum = <Cl2 as BindingAlgebra>::superpose(&left, &right).unwrap();
    let scaled = <Cl2 as BindingAlgebra>::scale(&left, -2.0).unwrap();

    assert_coefficients_close(&sum.to_coefficients(), &expected_sum.to_coefficients());
    assert_coefficients_close(
        &scaled.to_coefficients(),
        &expected_scaled.to_coefficients(),
    );
}
