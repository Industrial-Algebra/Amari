// SPDX-License-Identifier: MIT OR Apache-2.0

//! Member B — fixture for cargo inspection tests.
//!
//! Uses renamed-core (package = "amari-core") as its amari dep alias.
//! For testing that renamed dependencies are recognized by their local alias.

use renamed_core::Multivector;

/// Member B function using the renamed amari-core dependency.
pub fn compute_b(x: Multivector<3, 0, 0>) -> Multivector<3, 0, 0> {
    // Uses the type path from renamed_core
    x
}
