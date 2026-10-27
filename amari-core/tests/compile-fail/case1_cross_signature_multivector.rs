// SPDX-License-Identifier: MIT OR Apache-2.0
// C2 static rejection — cross-signature multivector arithmetic.
// TOKEN: E0308|VerifiedMultivector
use amari_core::verified::VerifiedMultivector;
fn main() {
    let a = VerifiedMultivector::<f64, 2, 0, 0>::scalar(1.0).unwrap();
    let b = VerifiedMultivector::<f64, 0, 2, 0>::scalar(1.0).unwrap();
    let _ = a.add(&b); // Cl(2,0,0) + Cl(0,2,0): must NOT compile
}
