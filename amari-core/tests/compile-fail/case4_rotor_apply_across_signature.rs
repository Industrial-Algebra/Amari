// SPDX-License-Identifier: MIT OR Apache-2.0
// C2 static rejection — rotor applied to foreign-signature multivector.
// TOKEN: E0308|VerifiedMultivector
use amari_core::verified::{VerifiedMultivector, VerifiedRotor};
fn main() {
    let mv = VerifiedMultivector::<f64, 3, 0, 0>::scalar(1.0).unwrap();
    let rotor = VerifiedRotor::<f64, 2, 0, 0>::new(mv.clone()).unwrap();
    let _ = rotor.apply_to_multivector(&mv); // Cl(2,0,0) rotor on Cl(3,0,0) vector
}
