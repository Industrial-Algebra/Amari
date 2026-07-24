// SPDX-License-Identifier: MIT OR Apache-2.0

//! Demo example showing Amari geometric algebra usage with autodiff.
//!
//! This example demonstrates shortest path via tropical algebra
//! on WASM targets.

use amari::dual::DualNumber;
use amari_core::Vector;

fn main() {
    let v = Vector::<3, 0>::new(1.0, 2.0, 3.0);
    println!("Vector magnitude: {}", v.magnitude());
}
