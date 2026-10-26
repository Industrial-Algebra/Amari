// SPDX-License-Identifier: MIT OR Apache-2.0
// C2 positive control: matching types MUST compile (exit 0).
use amari_core::verified::{KVector, Vector, VerifiedMultivector, VerifiedRotor};
fn main() {
    let a = VerifiedMultivector::<f64, 3, 0, 0>::scalar(1.0).unwrap();
    let b = VerifiedMultivector::<f64, 3, 0, 0>::scalar(2.0).unwrap();
    let _ = a.add(&b);
    let mv = VerifiedMultivector::<f64, 3, 0, 0>::scalar(1.0).unwrap();
    let rotor = VerifiedRotor::<f64, 3, 0, 0>::new(mv.clone()).unwrap();
    let _ = rotor.compose(&rotor).unwrap();
    let _ = rotor.apply_to_multivector(&mv);
    let k1 = KVector::<f64, 1, 3, 0, 0>::from_multivector(mv.clone());
    let k1b = KVector::<f64, 1, 3, 0, 0>::from_multivector(mv);
    let _ = k1.inner_product(&k1b);
    let u = Vector::<f64, 2>::new(vec![1.0, 0.0]).unwrap();
    let w = Vector::<f64, 2>::new(vec![0.0, 1.0]).unwrap();
    let _ = u.add(&w);
    let _ = u.dot(&w);
}
