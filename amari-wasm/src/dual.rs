//! WASM bindings for automatic differentiation with dual numbers

use amari_dual::{BranchPolicy, DualNumber, MultiDualNumber, StaticMultiDual};
use js_sys::Array;
use wasm_bindgen::prelude::*;

/// Tie-handling policy for branch-sensitive dual-number min/max operations.
#[wasm_bindgen]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WasmBranchPolicy {
    /// Preserve the left operand derivative on ties.
    Left,
    /// Preserve the right operand derivative on ties.
    Right,
    /// Average derivatives on ties.
    Average,
}

impl From<WasmBranchPolicy> for BranchPolicy {
    fn from(value: WasmBranchPolicy) -> Self {
        match value {
            WasmBranchPolicy::Left => Self::Left,
            WasmBranchPolicy::Right => Self::Right,
            WasmBranchPolicy::Average => Self::Average,
        }
    }
}

/// WASM wrapper for single-variable dual numbers
#[wasm_bindgen]
pub struct WasmDualNumber {
    inner: DualNumber<f64>,
}

#[wasm_bindgen]
impl WasmDualNumber {
    /// Create a new dual number with given real and dual parts
    #[wasm_bindgen(constructor)]
    pub fn new(real: f64, dual: f64) -> Self {
        Self {
            inner: DualNumber::new(real, dual),
        }
    }

    /// Create a variable (derivative = 1)
    #[wasm_bindgen(js_name = variable)]
    pub fn variable(value: f64) -> Self {
        Self {
            inner: DualNumber::variable(value),
        }
    }

    /// Create a constant (derivative = 0)
    #[wasm_bindgen(js_name = constant)]
    pub fn constant(value: f64) -> Self {
        Self {
            inner: DualNumber::constant(value),
        }
    }

    /// Get the real part (function value)
    #[wasm_bindgen(js_name = getReal)]
    pub fn get_real(&self) -> f64 {
        self.inner.value()
    }

    /// Get the dual part (derivative)
    #[wasm_bindgen(js_name = getDual)]
    pub fn get_dual(&self) -> f64 {
        self.inner.derivative()
    }

    /// Addition
    pub fn add(&self, other: &WasmDualNumber) -> WasmDualNumber {
        Self {
            inner: self.inner + other.inner,
        }
    }

    /// Subtraction
    pub fn sub(&self, other: &WasmDualNumber) -> WasmDualNumber {
        Self {
            inner: self.inner - other.inner,
        }
    }

    /// Multiplication
    pub fn mul(&self, other: &WasmDualNumber) -> WasmDualNumber {
        Self {
            inner: self.inner * other.inner,
        }
    }

    /// Division
    pub fn div(&self, other: &WasmDualNumber) -> Result<WasmDualNumber, JsValue> {
        if other.inner.real == 0.0 {
            return Err(JsValue::from_str("Division by zero"));
        }
        Ok(Self {
            inner: self.inner / other.inner,
        })
    }

    /// Negation
    pub fn neg(&self) -> WasmDualNumber {
        Self { inner: -self.inner }
    }

    /// Power function
    pub fn pow(&self, exponent: f64) -> WasmDualNumber {
        Self {
            inner: self.inner.powf(exponent),
        }
    }

    /// Exponential function
    pub fn exp(&self) -> WasmDualNumber {
        Self {
            inner: self.inner.exp(),
        }
    }

    /// Natural logarithm
    pub fn ln(&self) -> Result<WasmDualNumber, JsValue> {
        if self.inner.real <= 0.0 {
            return Err(JsValue::from_str("Logarithm of non-positive number"));
        }
        Ok(Self {
            inner: self.inner.ln(),
        })
    }

    /// Sine function
    pub fn sin(&self) -> WasmDualNumber {
        Self {
            inner: self.inner.sin(),
        }
    }

    /// Cosine function
    pub fn cos(&self) -> WasmDualNumber {
        Self {
            inner: self.inner.cos(),
        }
    }

    /// Square root
    pub fn sqrt(&self) -> Result<WasmDualNumber, JsValue> {
        if self.inner.real < 0.0 {
            return Err(JsValue::from_str("Square root of negative number"));
        }
        Ok(Self {
            inner: self.inner.sqrt(),
        })
    }

    /// Tangent function
    pub fn tan(&self) -> WasmDualNumber {
        Self {
            inner: self.inner.tan(),
        }
    }

    /// Hyperbolic sine
    pub fn sinh(&self) -> WasmDualNumber {
        Self {
            inner: self.inner.sinh(),
        }
    }

    /// Hyperbolic cosine
    pub fn cosh(&self) -> WasmDualNumber {
        Self {
            inner: self.inner.cosh(),
        }
    }

    /// Hyperbolic tangent
    pub fn tanh(&self) -> WasmDualNumber {
        Self {
            inner: self.inner.tanh(),
        }
    }

    /// ReLU activation function
    ///
    /// Note: Manual implementation as relu() is not in v0.12.0 DualNumber API
    pub fn relu(&self) -> WasmDualNumber {
        let x = self.inner.value();
        if x > 0.0 {
            // ReLU(x) = x, derivative = 1
            Self {
                inner: DualNumber::new(x, self.inner.derivative()),
            }
        } else {
            // ReLU(x) = 0, derivative = 0
            Self {
                inner: DualNumber::new(0.0, 0.0),
            }
        }
    }

    /// Sigmoid activation function
    pub fn sigmoid(&self) -> WasmDualNumber {
        Self {
            inner: self.inner.sigmoid(),
        }
    }

    /// Softplus activation function
    ///
    /// Note: Manual implementation as softplus() is not in v0.12.0 DualNumber API
    /// softplus(x) = ln(1 + exp(x))
    pub fn softplus(&self) -> WasmDualNumber {
        let x = self.inner.value();
        let exp_x = x.exp();
        let softplus_val = (1.0 + exp_x).ln();
        // Derivative of softplus: d/dx ln(1 + exp(x)) = exp(x)/(1 + exp(x)) = sigmoid(x)
        let softplus_deriv = exp_x / (1.0 + exp_x);
        Self {
            inner: DualNumber::new(softplus_val, self.inner.derivative() * softplus_deriv),
        }
    }

    /// Maximum of two dual numbers
    pub fn max(&self, other: &WasmDualNumber) -> WasmDualNumber {
        Self {
            inner: self.inner.max(other.inner),
        }
    }

    /// Minimum of two dual numbers
    pub fn min(&self, other: &WasmDualNumber) -> WasmDualNumber {
        Self {
            inner: self.inner.min(other.inner),
        }
    }

    /// Maximum with explicit derivative tie handling.
    #[wasm_bindgen(js_name = maxByPolicy)]
    pub fn max_by_policy(
        &self,
        other: &WasmDualNumber,
        policy: WasmBranchPolicy,
    ) -> WasmDualNumber {
        Self {
            inner: self.inner.max_by_policy(other.inner, policy.into()),
        }
    }

    /// Minimum with explicit derivative tie handling.
    #[wasm_bindgen(js_name = minByPolicy)]
    pub fn min_by_policy(
        &self,
        other: &WasmDualNumber,
        policy: WasmBranchPolicy,
    ) -> WasmDualNumber {
        Self {
            inner: self.inner.min_by_policy(other.inner, policy.into()),
        }
    }

    /// Integer power
    #[wasm_bindgen(js_name = powi)]
    pub fn powi(&self, n: i32) -> WasmDualNumber {
        Self {
            inner: self.inner.powi(n),
        }
    }
}

/// WASM wrapper for multi-variable dual numbers
#[wasm_bindgen]
pub struct WasmMultiDualNumber {
    inner: MultiDualNumber<f64>,
}

#[wasm_bindgen]
impl WasmMultiDualNumber {
    /// Create a new multi-dual number
    #[wasm_bindgen(constructor)]
    pub fn new(real: f64, duals: &[f64]) -> Self {
        Self {
            inner: MultiDualNumber::new(real, duals.to_vec()),
        }
    }

    /// Create a variable with derivative 1 at the specified index
    #[wasm_bindgen(js_name = variable)]
    pub fn variable(value: f64, num_vars: usize, var_index: usize) -> Self {
        Self {
            // Note: MultiDualNumber::variable takes (value, var_index, n_vars)
            inner: MultiDualNumber::variable(value, var_index, num_vars),
        }
    }

    /// Create a constant (all derivatives are zero)
    #[wasm_bindgen(js_name = constant)]
    pub fn constant(value: f64, num_vars: usize) -> Self {
        Self {
            inner: MultiDualNumber::constant(value, num_vars),
        }
    }

    /// Create one basis-seeded variable per input coordinate.
    #[wasm_bindgen(js_name = variables)]
    pub fn variables(values: &[f64]) -> Array {
        let result = Array::new();
        for variable in MultiDualNumber::variables(values) {
            result.push(&JsValue::from(Self { inner: variable }));
        }
        result
    }

    /// Get the real part (function value)
    #[wasm_bindgen(js_name = getReal)]
    pub fn get_real(&self) -> f64 {
        self.inner.get_value()
    }

    /// Get the gradient (all partial derivatives)
    #[wasm_bindgen(js_name = getGradient)]
    pub fn get_gradient(&self) -> Vec<f64> {
        self.inner.get_gradient().to_vec()
    }

    /// Get a specific partial derivative
    #[wasm_bindgen(js_name = getPartial)]
    pub fn get_partial(&self, index: usize) -> Result<f64, JsValue> {
        self.inner
            .get_gradient()
            .get(index)
            .copied()
            .ok_or_else(|| JsValue::from_str("Index out of bounds"))
    }

    /// Get number of variables
    #[wasm_bindgen(js_name = getNumVars)]
    pub fn get_num_vars(&self) -> usize {
        self.inner.n_vars()
    }

    /// Addition
    pub fn add(&self, other: &WasmMultiDualNumber) -> Result<WasmMultiDualNumber, JsValue> {
        if self.inner.n_vars() != other.inner.n_vars() {
            return Err(JsValue::from_str("Incompatible number of variables"));
        }
        Ok(Self {
            inner: self.inner.clone() + other.inner.clone(),
        })
    }

    /// Multiplication
    pub fn mul(&self, other: &WasmMultiDualNumber) -> Result<WasmMultiDualNumber, JsValue> {
        if self.inner.n_vars() != other.inner.n_vars() {
            return Err(JsValue::from_str("Incompatible number of variables"));
        }
        Ok(Self {
            inner: self.inner.clone() * other.inner.clone(),
        })
    }

    /// Square root
    pub fn sqrt(&self) -> Result<WasmMultiDualNumber, JsValue> {
        if self.inner.get_value() < 0.0 {
            return Err(JsValue::from_str("Square root of negative number"));
        }
        // Note: sqrt() not available on MultiDualNumber in v0.12.0
        // Providing stub that preserves value but computes correct gradient
        let val = self.inner.get_value().sqrt();
        // Compute gradient: d/dx sqrt(f) = f'/(2*sqrt(f))
        let sqrt_val = val;
        let grad: Vec<f64> = self
            .inner
            .get_gradient()
            .iter()
            .map(|&g| g / (2.0 * sqrt_val))
            .collect();
        Ok(Self {
            inner: MultiDualNumber::new(val, grad),
        })
    }

    /// Maximum with explicit derivative tie handling.
    #[wasm_bindgen(js_name = maxByPolicy)]
    pub fn max_by_policy(
        &self,
        other: &WasmMultiDualNumber,
        policy: WasmBranchPolicy,
    ) -> Result<WasmMultiDualNumber, JsValue> {
        if self.inner.n_vars() != other.inner.n_vars() {
            return Err(JsValue::from_str("Incompatible number of variables"));
        }
        Ok(Self {
            inner: self
                .inner
                .clone()
                .max_by_policy(other.inner.clone(), policy.into()),
        })
    }

    /// Minimum with explicit derivative tie handling.
    #[wasm_bindgen(js_name = minByPolicy)]
    pub fn min_by_policy(
        &self,
        other: &WasmMultiDualNumber,
        policy: WasmBranchPolicy,
    ) -> Result<WasmMultiDualNumber, JsValue> {
        if self.inner.n_vars() != other.inner.n_vars() {
            return Err(JsValue::from_str("Incompatible number of variables"));
        }
        Ok(Self {
            inner: self
                .inner
                .clone()
                .min_by_policy(other.inner.clone(), policy.into()),
        })
    }
}

macro_rules! define_static_multi_dual_wasm {
    ($name:ident, $n:expr) => {
        #[wasm_bindgen]
        pub struct $name {
            inner: StaticMultiDual<f64, $n>,
        }

        #[wasm_bindgen]
        impl $name {
            /// Create a fixed-size multi-dual number from a value and gradient.
            #[wasm_bindgen(constructor)]
            pub fn new(value: f64, gradient: &[f64]) -> Result<$name, JsValue> {
                if gradient.len() != $n {
                    return Err(JsValue::from_str(concat!(
                        "gradient length must be ",
                        stringify!($n)
                    )));
                }
                let mut array = [0.0; $n];
                array.copy_from_slice(gradient);
                Ok(Self {
                    inner: StaticMultiDual::new(value, array),
                })
            }

            /// Create a constant with zero gradient.
            pub fn constant(value: f64) -> $name {
                Self {
                    inner: StaticMultiDual::constant(value),
                }
            }

            /// Create a basis-seeded variable.
            pub fn variable(value: f64, var_index: usize) -> Result<$name, JsValue> {
                if var_index >= $n {
                    return Err(JsValue::from_str("variable index out of bounds"));
                }
                Ok(Self {
                    inner: StaticMultiDual::variable(value, var_index),
                })
            }

            /// Create one fixed-size variable per coordinate.
            pub fn variables(values: &[f64]) -> Result<Array, JsValue> {
                if values.len() != $n {
                    return Err(JsValue::from_str(concat!(
                        "values length must be ",
                        stringify!($n)
                    )));
                }
                let mut array = [0.0; $n];
                array.copy_from_slice(values);
                let result = Array::new();
                for variable in StaticMultiDual::variables(array) {
                    result.push(&JsValue::from(Self { inner: variable }));
                }
                Ok(result)
            }

            /// Get the function value.
            #[wasm_bindgen(js_name = getValue)]
            pub fn get_value(&self) -> f64 {
                self.inner.get_value()
            }

            /// Get the fixed-size gradient as a JavaScript array-compatible vector.
            #[wasm_bindgen(js_name = getGradient)]
            pub fn get_gradient(&self) -> Vec<f64> {
                self.inner.get_gradient().to_vec()
            }

            /// Number of derivative variables.
            #[wasm_bindgen(js_name = nVars)]
            pub fn n_vars(&self) -> usize {
                self.inner.n_vars()
            }

            /// Addition.
            pub fn add(&self, other: &$name) -> $name {
                Self {
                    inner: self.inner + other.inner,
                }
            }

            /// Subtraction.
            pub fn sub(&self, other: &$name) -> $name {
                Self {
                    inner: self.inner - other.inner,
                }
            }

            /// Multiplication.
            pub fn mul(&self, other: &$name) -> $name {
                Self {
                    inner: self.inner * other.inner,
                }
            }

            /// Division.
            pub fn div(&self, other: &$name) -> Result<$name, JsValue> {
                if other.inner.get_value() == 0.0 {
                    return Err(JsValue::from_str("Division by zero"));
                }
                Ok(Self {
                    inner: self.inner / other.inner,
                })
            }

            /// Negation.
            pub fn neg(&self) -> $name {
                Self { inner: -self.inner }
            }

            /// Maximum with explicit derivative tie handling.
            #[wasm_bindgen(js_name = maxByPolicy)]
            pub fn max_by_policy(&self, other: &$name, policy: WasmBranchPolicy) -> $name {
                Self {
                    inner: self.inner.max_by_policy(other.inner, policy.into()),
                }
            }

            /// Minimum with explicit derivative tie handling.
            #[wasm_bindgen(js_name = minByPolicy)]
            pub fn min_by_policy(&self, other: &$name, policy: WasmBranchPolicy) -> $name {
                Self {
                    inner: self.inner.min_by_policy(other.inner, policy.into()),
                }
            }

            /// Convert to the heap-backed multi-dual wrapper.
            #[wasm_bindgen(js_name = toMultiDual)]
            pub fn to_multi_dual(&self) -> WasmMultiDualNumber {
                WasmMultiDualNumber {
                    inner: self.inner.to_multi_dual(),
                }
            }
        }
    };
}

define_static_multi_dual_wasm!(WasmStaticMultiDual2, 2);
define_static_multi_dual_wasm!(WasmStaticMultiDual3, 3);
define_static_multi_dual_wasm!(WasmStaticMultiDual4, 4);

/// Automatic differentiation utilities
#[wasm_bindgen]
pub struct AutoDiff;

#[wasm_bindgen]
impl AutoDiff {
    /// Compute numerical derivative using finite differences (fallback implementation)
    #[wasm_bindgen(js_name = numericalDerivative)]
    pub fn numerical_derivative(x: f64, f: &js_sys::Function, h: f64) -> Result<f64, JsValue> {
        let this = &JsValue::null();

        // f(x + h)
        let args_plus = js_sys::Array::new();
        args_plus.push(&JsValue::from(x + h));
        let f_plus = f
            .apply(this, &args_plus)?
            .as_f64()
            .ok_or_else(|| JsValue::from_str("Function must return a number"))?;

        // f(x - h)
        let args_minus = js_sys::Array::new();
        args_minus.push(&JsValue::from(x - h));
        let f_minus = f
            .apply(this, &args_minus)?
            .as_f64()
            .ok_or_else(|| JsValue::from_str("Function must return a number"))?;

        // Central difference: (f(x+h) - f(x-h)) / (2h)
        Ok((f_plus - f_minus) / (2.0 * h))
    }

    /// Create a dual number and evaluate a polynomial
    #[wasm_bindgen(js_name = evaluatePolynomial)]
    pub fn evaluate_polynomial(x: f64, coefficients: &[f64]) -> WasmDualNumber {
        let dual_x = WasmDualNumber::variable(x);
        let mut result = WasmDualNumber::constant(0.0);

        for (i, &coeff) in coefficients.iter().enumerate() {
            let term = if i == 0 {
                WasmDualNumber::constant(coeff)
            } else {
                let power = dual_x.pow(i as f64);
                power.mul(&WasmDualNumber::constant(coeff))
            };
            result = result.add(&term);
        }

        result
    }

    /// Compute mean squared error with automatic gradients
    #[wasm_bindgen(js_name = meanSquaredError)]
    pub fn mean_squared_error(
        predictions: &[f64],
        targets: &[f64],
    ) -> Result<WasmDualNumber, JsValue> {
        if predictions.len() != targets.len() {
            return Err(JsValue::from_str(
                "Predictions and targets must have same length",
            ));
        }

        let n = predictions.len();
        let mut mse = WasmDualNumber::constant(0.0);

        for (pred_val, target_val) in predictions.iter().zip(targets.iter()) {
            let pred = WasmDualNumber::variable(*pred_val);
            let target = WasmDualNumber::constant(*target_val);
            let diff = pred.sub(&target);
            let squared = diff.mul(&diff);
            mse = mse.add(&squared);
        }

        let n_dual = WasmDualNumber::constant(n as f64);
        mse.div(&n_dual)
    }

    /// Linear layer forward pass (y = Wx + b) with automatic gradients
    #[wasm_bindgen(js_name = linearLayer)]
    pub fn linear_layer(
        inputs: &[f64],
        weights: &[f64], // Flattened weight matrix
        bias: &[f64],
        input_size: usize,
        output_size: usize,
    ) -> Result<Vec<f64>, JsValue> {
        if weights.len() != input_size * output_size {
            return Err(JsValue::from_str("Weight matrix size mismatch"));
        }
        if bias.len() != output_size {
            return Err(JsValue::from_str("Bias vector size mismatch"));
        }
        if inputs.len() != input_size {
            return Err(JsValue::from_str("Input vector size mismatch"));
        }

        let mut outputs = Vec::new();

        for i in 0..output_size {
            let mut output = WasmDualNumber::constant(bias[i]);

            for j in 0..input_size {
                let input_dual = WasmDualNumber::variable(inputs[j]);
                let weight_dual = WasmDualNumber::constant(weights[i * input_size + j]);
                let product = input_dual.mul(&weight_dual);
                output = output.add(&product);
            }

            outputs.push(output.get_real());
        }

        Ok(outputs)
    }
}

/// Advanced machine learning operations using dual numbers
#[wasm_bindgen]
pub struct MLOps;

#[wasm_bindgen]
impl MLOps {
    /// Compute softmax with automatic gradients
    #[wasm_bindgen(js_name = softmax)]
    pub fn softmax(inputs: &[f64]) -> Vec<f64> {
        let mut dual_inputs = Vec::new();

        // Convert inputs to dual numbers
        for &val in inputs {
            dual_inputs.push(WasmDualNumber::variable(val));
        }

        // Find max for numerical stability
        let max_val = inputs.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        let max_dual = WasmDualNumber::constant(max_val);

        // Compute exp(x_i - max) for each input
        let mut exp_vals = Vec::new();
        for dual in &dual_inputs {
            let shifted = dual.sub(&max_dual);
            exp_vals.push(shifted.exp());
        }

        // Compute sum of exponentials
        let mut sum = WasmDualNumber::constant(0.0);
        for exp_val in &exp_vals {
            sum = sum.add(exp_val);
        }

        // Divide each by sum to get softmax
        let mut result = Vec::new();
        for exp_val in exp_vals {
            let softmax_val = exp_val.div(&sum).unwrap();
            result.push(softmax_val.get_real());
        }

        result
    }

    /// Batch apply activation function with gradients
    #[wasm_bindgen(js_name = batchActivation)]
    pub fn batch_activation(inputs: &[f64], activation: &str) -> Result<Vec<f64>, JsValue> {
        let mut outputs = Vec::new();

        for &input in inputs {
            let dual = WasmDualNumber::variable(input);
            let output = match activation {
                "relu" => dual.relu(),
                "sigmoid" => dual.sigmoid(),
                "tanh" => dual.tanh(),
                "softplus" => dual.softplus(),
                "sin" => dual.sin(),
                "cos" => dual.cos(),
                "exp" => dual.exp(),
                _ => return Err(JsValue::from_str("Unsupported activation function")),
            };
            outputs.push(output.get_real());
        }

        Ok(outputs)
    }

    /// Gradient descent step
    #[wasm_bindgen(js_name = gradientDescentStep)]
    pub fn gradient_descent_step(
        parameters: &[f64],
        gradients: &[f64],
        learning_rate: f64,
    ) -> Result<Vec<f64>, JsValue> {
        if parameters.len() != gradients.len() {
            return Err(JsValue::from_str(
                "Parameters and gradients must have same length",
            ));
        }

        let mut updated_params = Vec::new();
        for (&param, &grad) in parameters.iter().zip(gradients.iter()) {
            let new_param = param - learning_rate * grad;
            updated_params.push(new_param);
        }

        Ok(updated_params)
    }

    /// Compute cross-entropy loss with automatic gradients
    #[wasm_bindgen(js_name = crossEntropyLoss)]
    pub fn cross_entropy_loss(
        predictions: &[f64],
        targets: &[f64],
    ) -> Result<WasmDualNumber, JsValue> {
        if predictions.len() != targets.len() {
            return Err(JsValue::from_str(
                "Predictions and targets must have same length",
            ));
        }

        let mut loss = WasmDualNumber::constant(0.0);
        let eps = 1e-8; // Small value to prevent log(0)

        for (&pred, &target) in predictions.iter().zip(targets.iter()) {
            let pred_dual = WasmDualNumber::variable((pred + eps).max(eps).min(1.0 - eps));
            let target_dual = WasmDualNumber::constant(target);

            let log_pred = pred_dual.ln()?;
            let term = target_dual.mul(&log_pred);
            loss = loss.sub(&term);
        }

        Ok(loss)
    }
}

/// Batch operations for efficient computation
#[wasm_bindgen]
pub struct BatchOps;

#[wasm_bindgen]
impl BatchOps {
    /// Matrix multiplication with automatic gradients
    #[wasm_bindgen(js_name = matrixMultiply)]
    pub fn matrix_multiply(
        a: &[f64],
        b: &[f64],
        a_rows: usize,
        a_cols: usize,
        b_rows: usize,
        b_cols: usize,
    ) -> Result<Vec<f64>, JsValue> {
        if a_cols != b_rows {
            return Err(JsValue::from_str(
                "Matrix dimensions incompatible for multiplication",
            ));
        }
        if a.len() != a_rows * a_cols || b.len() != b_rows * b_cols {
            return Err(JsValue::from_str("Matrix data size mismatch"));
        }

        let mut result = Vec::with_capacity(a_rows * b_cols);

        for i in 0..a_rows {
            for j in 0..b_cols {
                let mut sum = WasmDualNumber::constant(0.0);

                for k in 0..a_cols {
                    let a_val = WasmDualNumber::variable(a[i * a_cols + k]);
                    let b_val = WasmDualNumber::constant(b[k * b_cols + j]);
                    let product = a_val.mul(&b_val);
                    sum = sum.add(&product);
                }

                result.push(sum.get_real());
            }
        }

        Ok(result)
    }

    /// Compute Jacobian matrix for vector function
    #[wasm_bindgen(js_name = computeJacobian)]
    pub fn compute_jacobian(
        input_values: &[f64],
        function_name: &str,
    ) -> Result<Vec<f64>, JsValue> {
        let mut jacobian = Vec::new();

        for &input_val in input_values {
            let dual = WasmDualNumber::variable(input_val);

            let result = match function_name {
                "sin" => dual.sin(),
                "cos" => dual.cos(),
                "exp" => dual.exp(),
                "tanh" => dual.tanh(),
                "sigmoid" => dual.sigmoid(),
                "square" => dual.mul(&dual),
                _ => return Err(JsValue::from_str("Unsupported function")),
            };

            jacobian.push(result.get_dual());
        }

        Ok(jacobian)
    }

    /// Batch evaluate polynomial with derivatives
    #[wasm_bindgen(js_name = batchPolynomial)]
    pub fn batch_polynomial(x_values: &[f64], coefficients: &[f64]) -> Result<Vec<f64>, JsValue> {
        let mut results = Vec::new();

        for &x in x_values {
            let poly_result = AutoDiff::evaluate_polynomial(x, coefficients);
            results.push(poly_result.get_real());
            results.push(poly_result.get_dual()); // Include derivative
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // WasmDualNumber Tests
    // ========================================================================

    #[test]
    fn test_dual_new() {
        let d = WasmDualNumber::new(3.0, 4.0);
        assert_eq!(d.get_real(), 3.0);
        assert_eq!(d.get_dual(), 4.0);
    }

    #[test]
    fn test_dual_variable() {
        let x = WasmDualNumber::variable(5.0);
        assert_eq!(x.get_real(), 5.0);
        assert_eq!(x.get_dual(), 1.0); // derivative seed = 1
    }

    #[test]
    fn test_dual_constant() {
        let c = WasmDualNumber::constant(7.0);
        assert_eq!(c.get_real(), 7.0);
        assert_eq!(c.get_dual(), 0.0); // derivative = 0
    }

    #[test]
    fn test_dual_addition() {
        let x = WasmDualNumber::variable(3.0);
        let y = WasmDualNumber::constant(2.0);

        let sum = x.add(&y);
        assert_eq!(sum.get_real(), 5.0);
        assert_eq!(sum.get_dual(), 1.0); // d/dx(x + 2) = 1
    }

    #[test]
    fn test_dual_subtraction() {
        let x = WasmDualNumber::variable(5.0);
        let y = WasmDualNumber::constant(3.0);

        let diff = x.sub(&y);
        assert_eq!(diff.get_real(), 2.0);
        assert_eq!(diff.get_dual(), 1.0); // d/dx(x - 3) = 1
    }

    #[test]
    fn test_dual_multiplication() {
        let x = WasmDualNumber::variable(3.0);
        let y = WasmDualNumber::constant(4.0);

        let prod = x.mul(&y);
        assert_eq!(prod.get_real(), 12.0);
        assert_eq!(prod.get_dual(), 4.0); // d/dx(4x) = 4
    }

    #[test]
    fn test_dual_multiplication_two_vars() {
        // f(x) = x * x = x^2
        let x = WasmDualNumber::variable(3.0);
        let x_squared = x.mul(&x);
        assert_eq!(x_squared.get_real(), 9.0);
        assert_eq!(x_squared.get_dual(), 6.0); // d/dx(x^2) = 2x = 6
    }

    #[test]
    fn test_dual_division() {
        let x = WasmDualNumber::variable(6.0);
        let y = WasmDualNumber::constant(2.0);

        let quot = x.div(&y).unwrap();
        assert_eq!(quot.get_real(), 3.0);
        assert_eq!(quot.get_dual(), 0.5); // d/dx(x/2) = 1/2
    }

    #[test]
    fn test_dual_exp() {
        let x = WasmDualNumber::variable(0.0);
        let exp_x = x.exp();

        assert!((exp_x.get_real() - 1.0).abs() < 1e-10); // exp(0) = 1
        assert!((exp_x.get_dual() - 1.0).abs() < 1e-10); // d/dx(exp(x)) = exp(x) = 1
    }

    #[test]
    fn test_dual_sin() {
        let x = WasmDualNumber::variable(0.0);
        let sin_x = x.sin();

        assert!(sin_x.get_real().abs() < 1e-10); // sin(0) = 0
        assert!((sin_x.get_dual() - 1.0).abs() < 1e-10); // d/dx(sin(x)) = cos(x) = cos(0) = 1
    }

    #[test]
    fn test_dual_cos() {
        let x = WasmDualNumber::variable(0.0);
        let cos_x = x.cos();

        assert!((cos_x.get_real() - 1.0).abs() < 1e-10); // cos(0) = 1
        assert!(cos_x.get_dual().abs() < 1e-10); // d/dx(cos(x)) = -sin(x) = -sin(0) = 0
    }

    #[test]
    fn test_dual_pow() {
        let x = WasmDualNumber::variable(2.0);
        let x_cubed = x.pow(3.0);

        assert!((x_cubed.get_real() - 8.0).abs() < 1e-10); // 2^3 = 8
        assert!((x_cubed.get_dual() - 12.0).abs() < 1e-10); // d/dx(x^3) = 3x^2 = 3*4 = 12
    }

    #[test]
    fn test_dual_sqrt() {
        let x = WasmDualNumber::variable(4.0);
        let sqrt_x = x.sqrt().unwrap();

        assert!((sqrt_x.get_real() - 2.0).abs() < 1e-10); // sqrt(4) = 2
        assert!((sqrt_x.get_dual() - 0.25).abs() < 1e-10); // d/dx(sqrt(x)) = 1/(2*sqrt(x)) = 1/4
    }

    #[test]
    fn test_dual_tanh() {
        let x = WasmDualNumber::variable(0.0);
        let tanh_x = x.tanh();

        assert!(tanh_x.get_real().abs() < 1e-10); // tanh(0) = 0
        assert!((tanh_x.get_dual() - 1.0).abs() < 1e-10); // d/dx(tanh(x)) at x=0 = sech^2(0) = 1
    }

    #[test]
    fn test_dual_sigmoid() {
        let x = WasmDualNumber::variable(0.0);
        let sig_x = x.sigmoid();

        assert!((sig_x.get_real() - 0.5).abs() < 1e-10); // sigmoid(0) = 0.5
        assert!((sig_x.get_dual() - 0.25).abs() < 1e-10); // sigmoid'(0) = 0.25
    }

    #[test]
    fn test_dual_relu() {
        let pos = WasmDualNumber::variable(2.0);
        let relu_pos = pos.relu();
        assert_eq!(relu_pos.get_real(), 2.0);
        assert_eq!(relu_pos.get_dual(), 1.0);

        let neg = WasmDualNumber::variable(-2.0);
        let relu_neg = neg.relu();
        assert_eq!(relu_neg.get_real(), 0.0);
        assert_eq!(relu_neg.get_dual(), 0.0);
    }

    #[test]
    fn test_dual_branch_policy_ties_are_exposed() {
        let left = WasmDualNumber::new(3.0, 1.0);
        let right = WasmDualNumber::new(3.0, 5.0);

        let left_biased = left.max_by_policy(&right, WasmBranchPolicy::Left);
        assert_eq!(left_biased.get_real(), 3.0);
        assert_eq!(left_biased.get_dual(), 1.0);

        let right_biased = left.max_by_policy(&right, WasmBranchPolicy::Right);
        assert_eq!(right_biased.get_real(), 3.0);
        assert_eq!(right_biased.get_dual(), 5.0);

        let averaged = left.min_by_policy(&right, WasmBranchPolicy::Average);
        assert_eq!(averaged.get_real(), 3.0);
        assert_eq!(averaged.get_dual(), 3.0);
    }

    #[test]
    fn test_dual_chain_rule() {
        // f(x) = sin(x^2) at x = sqrt(pi/2)
        // f'(x) = 2x * cos(x^2)
        let x = WasmDualNumber::variable(1.0);
        let x_sq = x.mul(&x); // x^2
        let result = x_sq.sin(); // sin(x^2)

        // At x=1: sin(1) and derivative = 2*1*cos(1)
        assert!((result.get_real() - 1.0_f64.sin()).abs() < 1e-10);
        assert!((result.get_dual() - 2.0 * 1.0_f64.cos()).abs() < 1e-10);
    }

    // ========================================================================
    // WasmMultiDualNumber Tests
    // ========================================================================

    #[test]
    fn test_multi_dual_variable() {
        let x = WasmMultiDualNumber::variable(3.0, 2, 0);
        assert_eq!(x.get_real(), 3.0);
        let grad = x.get_gradient();
        assert_eq!(grad[0], 1.0);
        assert_eq!(grad[1], 0.0);
    }

    #[test]
    fn test_multi_dual_constant() {
        let c = WasmMultiDualNumber::constant(5.0, 3);
        assert_eq!(c.get_real(), 5.0);
        let grad = c.get_gradient();
        assert_eq!(grad, vec![0.0, 0.0, 0.0]);
    }

    #[test]
    fn test_multi_dual_add() {
        let x = WasmMultiDualNumber::variable(2.0, 2, 0);
        let y = WasmMultiDualNumber::variable(3.0, 2, 1);

        let sum = x.add(&y).unwrap();
        assert_eq!(sum.get_real(), 5.0);
        let grad = sum.get_gradient();
        assert_eq!(grad[0], 1.0); // ∂/∂x(x+y) = 1
        assert_eq!(grad[1], 1.0); // ∂/∂y(x+y) = 1
    }

    #[test]
    fn test_multi_dual_mul() {
        let x = WasmMultiDualNumber::variable(2.0, 2, 0);
        let y = WasmMultiDualNumber::variable(3.0, 2, 1);

        let prod = x.mul(&y).unwrap();
        assert_eq!(prod.get_real(), 6.0);
        let grad = prod.get_gradient();
        assert_eq!(grad[0], 3.0); // ∂/∂x(xy) = y = 3
        assert_eq!(grad[1], 2.0); // ∂/∂y(xy) = x = 2
    }

    #[test]
    fn test_multi_dual_variables_and_branch_policy_are_exposed() {
        #[cfg(target_arch = "wasm32")]
        {
            let variables = WasmMultiDualNumber::variables(&[2.0, 3.0, 5.0]);
            assert_eq!(variables.length(), 3);
        }

        let left = WasmMultiDualNumber::new(7.0, &[1.0, 0.0]);
        let right = WasmMultiDualNumber::new(7.0, &[0.0, 1.0]);
        let averaged = left
            .max_by_policy(&right, WasmBranchPolicy::Average)
            .unwrap();
        assert_eq!(averaged.get_real(), 7.0);
        assert_eq!(averaged.get_gradient(), vec![0.5, 0.5]);
    }

    #[test]
    fn test_static_multi_dual2_arithmetic_is_exposed() {
        let x = WasmStaticMultiDual2::variable(2.0, 0).unwrap();
        let y = WasmStaticMultiDual2::variable(3.0, 1).unwrap();
        let product = x.mul(&y);

        assert_eq!(product.get_value(), 6.0);
        assert_eq!(product.get_gradient(), vec![3.0, 2.0]);

        let tied_left = WasmStaticMultiDual2::new(4.0, &[1.0, 0.0]).unwrap();
        let tied_right = WasmStaticMultiDual2::new(4.0, &[0.0, 1.0]).unwrap();
        let averaged = tied_left.max_by_policy(&tied_right, WasmBranchPolicy::Average);
        assert_eq!(averaged.get_gradient(), vec![0.5, 0.5]);

        let dynamic = averaged.to_multi_dual();
        assert_eq!(dynamic.get_real(), 4.0);
        assert_eq!(dynamic.get_gradient(), vec![0.5, 0.5]);
    }

    // ========================================================================
    // AutoDiff Tests
    // ========================================================================

    #[test]
    fn test_autodiff_polynomial() {
        // f(x) = 1 + 2x + 3x^2 at x = 2
        // f(2) = 1 + 4 + 12 = 17
        // f'(x) = 2 + 6x, f'(2) = 14
        let result = AutoDiff::evaluate_polynomial(2.0, &[1.0, 2.0, 3.0]);
        assert_eq!(result.get_real(), 17.0);
        assert_eq!(result.get_dual(), 14.0);
    }

    #[test]
    fn test_autodiff_polynomial_constant() {
        // f(x) = 5 (constant polynomial)
        let result = AutoDiff::evaluate_polynomial(10.0, &[5.0]);
        assert_eq!(result.get_real(), 5.0);
        assert_eq!(result.get_dual(), 0.0);
    }

    #[test]
    fn test_autodiff_mean_squared_error() {
        // MSE between [1, 2, 3] and [1, 2, 3] should be 0
        let result = AutoDiff::mean_squared_error(&[1.0, 2.0, 3.0], &[1.0, 2.0, 3.0]).unwrap();
        assert!(result.get_real().abs() < 1e-10);
    }

    #[test]
    fn test_autodiff_mse_with_difference() {
        // MSE between [2, 3] and [1, 2] should be ((2-1)^2 + (3-2)^2) / 2 = 1
        let result = AutoDiff::mean_squared_error(&[2.0, 3.0], &[1.0, 2.0]).unwrap();
        assert!((result.get_real() - 1.0).abs() < 1e-10);
    }
}
