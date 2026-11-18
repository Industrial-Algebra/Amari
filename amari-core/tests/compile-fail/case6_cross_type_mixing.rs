// SPDX-License-Identifier: MIT OR Apache-2.0
// C2 static rejection — branded-type mixing / non-conversion (D2 amendment).
// TOKEN: E0308|Vector|VerifiedMultivector
use amari_core::verified::{KVector, Vector, VerifiedMultivector};
fn main() {
    let v = Vector::<f64, 2>::new(vec![1.0, 0.0]).unwrap();
    // A Vector is not a VerifiedMultivector: no implicit path may exist.
    let _ = VerifiedMultivector::<f64, 1, 1, 0>::add(&v, &v);
    // KVector is not its wrapped VerifiedMultivector without the explicit
    // constructor:
    let mv = VerifiedMultivector::<f64, 2, 0, 0>::scalar(1.0).unwrap();
    let k = KVector::<f64, 1, 2, 0, 0>::from_multivector(mv.clone());
    let _ = mv.add(&k); // no From/Into bypass: must NOT compile
}
