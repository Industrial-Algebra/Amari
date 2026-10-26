// SPDX-License-Identifier: MIT OR Apache-2.0
// C2 static rejection — cross-signature rotor composition.
// TOKEN: E0308|VerifiedRotor
use amari_core::verified::{VerifiedMultivector, VerifiedRotor};
fn main() {
    let mv1 = VerifiedMultivector::<f64, 3, 0, 0>::scalar(1.0).unwrap();
    let mv2 = VerifiedMultivector::<f64, 0, 3, 0>::scalar(1.0).unwrap();
    let r1 = VerifiedRotor::<f64, 3, 0, 0>::new(mv1).unwrap();
    let r2 = VerifiedRotor::<f64, 0, 3, 0>::new(mv2).unwrap();
    let _ = r1.compose(&r2); // must NOT compile
}
