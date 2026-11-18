// SPDX-License-Identifier: MIT OR Apache-2.0
// C2 static rejection — grade mismatch through KVector branding.
// TOKEN: E0308|KVector
use amari_core::verified::{KVector, VerifiedMultivector};
fn main() {
    let mv = VerifiedMultivector::<f64, 3, 0, 0>::scalar(1.0).unwrap();
    let k1 = KVector::<f64, 1, 3, 0, 0>::from_multivector(mv.clone());
    let k2 = KVector::<f64, 2, 3, 0, 0>::from_multivector(mv);
    let _ = k1.inner_product(&k2); // grade 1 vs grade 2: must NOT compile
}
