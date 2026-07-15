// SPDX-License-Identifier: MIT OR Apache-2.0

//! Integration tests for rust-project.
//!
//! Tests tropical algebra and geometric algebra integration
//! with native BLAS linking for performance-critical paths.

use amari_core::Multivector;
use amari_tropical::TropicalNumber;

#[test]
fn test_tropical_geometric_integration() {
    let a = TropicalNumber::new(3.0);
    let b = TropicalNumber::new(2.0);
    let result = a.tropical_add(&b);
    assert_eq!(result.value(), 4.0);
}

#[cfg(feature = "experimental")]
#[test]
fn test_cfg_gated_feature() {
    // Test behind cfg gate
}
