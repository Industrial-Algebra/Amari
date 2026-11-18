// SPDX-License-Identifier: MIT OR Apache-2.0
// C2 static rejection — cross-dimension Vector arithmetic (D2 amendment).
// TOKEN: E0308|Vector
use amari_core::verified::Vector;
fn main() {
    let u = Vector::<f64, 2>::new(vec![1.0, 0.0]).unwrap();
    let v = Vector::<f64, 3>::new(vec![0.0, 1.0, 0.0]).unwrap();
    let _ = u.add(&v); // 2D + 3D: must NOT compile
    let _ = u.dot(&v); // and dot: must NOT compile
}
