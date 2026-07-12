// SPDX-License-Identifier: MIT OR Apache-2.0

//! Member A — fixture for cargo inspection tests.
//!
//! Uses amari-core and amari-tropical for geometric algebra
//! and tropical algebra with WASM target support.

#![cfg_attr(target_arch = "wasm32", no_std)]

use amari_core::Multivector;
use amari_tropical::TropicalNumber;

/// A member function using amari types in type position.
pub fn compute(x: TropicalNumber<f64>) -> f64 {
    x.value()
}

/// A member function using fully qualified path in expression context.
pub fn make_vector() -> amari_core::Vector<3, 0> {
    amari_core::Vector::new(1.0, 0.0, 0.0)
}

/// T is bounded by a trait from amari_core (simulating).
pub fn generic_op<T: amari_core::SomeTrait>(val: T) {}
