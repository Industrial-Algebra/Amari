// SPDX-License-Identifier: MIT OR Apache-2.0
// C2 static rejection — rotor applied to foreign-signature multivector.
// The rotor itself is VALIDLY constructed (grade-0 unit, Cl(2,0,0)); the
// ONLY boundary under test is apply_to_multivector's argument (R1 P2:
// the first draft put the mismatch in the constructor, masking the
// apply boundary — a scratch fix of the argument left the case red for
// the wrong reason).
// TOKEN: E0308|VerifiedMultivector
use amari_core::verified::{VerifiedMultivector, VerifiedRotor};
fn main() {
    let mv2 = VerifiedMultivector::<f64, 2, 0, 0>::scalar(1.0).unwrap();
    let rotor = VerifiedRotor::<f64, 2, 0, 0>::new(mv2).unwrap(); // valid
    let foreign = VerifiedMultivector::<f64, 3, 0, 0>::scalar(1.0).unwrap();
    let _ = rotor.apply_to_multivector(&foreign); // Cl(2,0,0) rotor on Cl(3,0,0) vector
}
