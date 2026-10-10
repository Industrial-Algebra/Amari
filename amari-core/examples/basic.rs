//! Basic usage example for the Amari geometric algebra library
//!
//! This example demonstrates:
//! 1. Creating 3D Euclidean Clifford algebra (3,0,0)
//! 2. Constructing basis vectors e1, e2, e3
//! 3. Computing e1 ∧ e2 (bivector)
//! 4. Creating a rotor from the bivector
//! 5. Applying rotor to rotate a vector

use amari_core::{
    basis::{Basis, MultivectorBuilder},
    rotor::Rotor,
    Multivector,
};

fn main() {
    println!("Amari Geometric Algebra Example");
    println!("==============================\n");

    // 1. Work with 3D Euclidean Clifford algebra Cl(3,0,0)
    type Cl3 = Multivector<3, 0, 0>;

    // 2. Create basis vectors e1, e2, e3
    let e1: Cl3 = Basis::e1();
    let e2: Cl3 = Basis::e2();
    let e3: Cl3 = Basis::e3();

    println!("Basis vectors:");
    println!(
        "e1 = [{:.1}, {:.1}, {:.1}, {:.1}, {:.1}, {:.1}, {:.1}, {:.1}]",
        e1.get(0),
        e1.get(1),
        e1.get(2),
        e1.get(3),
        e1.get(4),
        e1.get(5),
        e1.get(6),
        e1.get(7)
    );
    println!(
        "e2 = [{:.1}, {:.1}, {:.1}, {:.1}, {:.1}, {:.1}, {:.1}, {:.1}]",
        e2.get(0),
        e2.get(1),
        e2.get(2),
        e2.get(3),
        e2.get(4),
        e2.get(5),
        e2.get(6),
        e2.get(7)
    );
    println!(
        "e3 = [{:.1}, {:.1}, {:.1}, {:.1}, {:.1}, {:.1}, {:.1}, {:.1}]\n",
        e3.get(0),
        e3.get(1),
        e3.get(2),
        e3.get(3),
        e3.get(4),
        e3.get(5),
        e3.get(6),
        e3.get(7)
    );

    // Verify orthonormality
    println!("Orthonormality checks:");
    println!("e1 · e1 = {:.1}", e1.scalar_product(&e1));
    println!("e2 · e2 = {:.1}", e2.scalar_product(&e2));
    println!("e3 · e3 = {:.1}", e3.scalar_product(&e3));
    println!("e1 · e2 = {:.1}", e1.scalar_product(&e2));
    println!("e1 · e3 = {:.1}", e1.scalar_product(&e3));
    println!("e2 · e3 = {:.1}\n", e2.scalar_product(&e3));

    // 3. Compute e1 ∧ e2 (bivector representing the xy-plane)
    let e12 = e1.outer_product(&e2);
    println!("Bivector e1 ∧ e2:");
    println!(
        "e12 = [{:.1}, {:.1}, {:.1}, {:.1}, {:.1}, {:.1}, {:.1}, {:.1}]\n",
        e12.get(0),
        e12.get(1),
        e12.get(2),
        e12.get(3),
        e12.get(4),
        e12.get(5),
        e12.get(6),
        e12.get(7)
    );

    // Verify anticommutivity: e1 ∧ e2 = -e2 ∧ e1
    let e21 = e2.outer_product(&e1);
    println!("Anticommutivity check:");
    println!("e1 ∧ e2 = {:.1} (e12 component)", e12.get(3));
    println!("e2 ∧ e1 = {:.1} (e12 component)\n", e21.get(3));

    // 4. Create a rotor for 90-degree rotation in the e1-e2 plane
    let angle = std::f64::consts::PI / 2.0; // 90 degrees
    let rotor = Rotor::from_multivector_bivector(&e12, angle);

    println!("Rotor for 90° rotation in e1-e2 plane:");
    let rotor_mv = rotor.as_multivector();
    println!(
        "R = [{:.3}, {:.3}, {:.3}, {:.3}, {:.3}, {:.3}, {:.3}, {:.3}]",
        rotor_mv.get(0),
        rotor_mv.get(1),
        rotor_mv.get(2),
        rotor_mv.get(3),
        rotor_mv.get(4),
        rotor_mv.get(5),
        rotor_mv.get(6),
        rotor_mv.get(7)
    );
    println!(
        "Rotor coefficient norm: {:.6}\n",
        rotor_mv.coefficient_norm()
    );

    // 5. Apply rotor to rotate vectors
    println!("Rotation examples:");

    // Rotate e1 (should become e2)
    let rotated_e1 = rotor.apply(&e1);
    println!(
        "R · e1 · R† = [{:.3}, {:.3}, {:.3}, {:.3}, {:.3}, {:.3}, {:.3}, {:.3}]",
        rotated_e1.get(0),
        rotated_e1.get(1),
        rotated_e1.get(2),
        rotated_e1.get(3),
        rotated_e1.get(4),
        rotated_e1.get(5),
        rotated_e1.get(6),
        rotated_e1.get(7)
    );
    println!(
        "This should be approximately e2: {:.6}",
        (rotated_e1.clone() - e2.clone()).coefficient_norm()
    );

    // Rotate e2 (should become -e1)
    let rotated_e2 = rotor.apply(&e2);
    println!(
        "R · e2 · R† = [{:.3}, {:.3}, {:.3}, {:.3}, {:.3}, {:.3}, {:.3}, {:.3}]",
        rotated_e2.get(0),
        rotated_e2.get(1),
        rotated_e2.get(2),
        rotated_e2.get(3),
        rotated_e2.get(4),
        rotated_e2.get(5),
        rotated_e2.get(6),
        rotated_e2.get(7)
    );
    println!(
        "This should be approximately -e1: {:.6}",
        (rotated_e2.clone() + e1).coefficient_norm()
    );

    // e3 should be unchanged (rotation is in e1-e2 plane)
    let rotated_e3 = rotor.apply(&e3);
    println!(
        "R · e3 · R† = [{:.3}, {:.3}, {:.3}, {:.3}, {:.3}, {:.3}, {:.3}, {:.3}]",
        rotated_e3.get(0),
        rotated_e3.get(1),
        rotated_e3.get(2),
        rotated_e3.get(3),
        rotated_e3.get(4),
        rotated_e3.get(5),
        rotated_e3.get(6),
        rotated_e3.get(7)
    );
    println!(
        "This should be approximately e3: {:.6}\n",
        (rotated_e3 - e3).coefficient_norm()
    );

    // Demonstrate composition of rotations
    println!("Rotor composition:");
    let half_rotor = Rotor::from_multivector_bivector(&e12, angle / 2.0); // 45 degrees
    let composed = half_rotor.compose(&half_rotor); // 45 + 45 = 90 degrees

    let composed_mv = composed.as_multivector();
    println!("45° rotor composed with itself:");
    println!(
        "R₄₅ ∘ R₄₅ = [{:.3}, {:.3}, {:.3}, {:.3}, {:.3}, {:.3}, {:.3}, {:.3}]",
        composed_mv.get(0),
        composed_mv.get(1),
        composed_mv.get(2),
        composed_mv.get(3),
        composed_mv.get(4),
        composed_mv.get(5),
        composed_mv.get(6),
        composed_mv.get(7)
    );

    let difference = (composed_mv - rotor_mv).coefficient_norm();
    println!("Difference from 90° rotor: {:.6}\n", difference);

    // Demonstrate using the builder pattern
    println!("Using MultivectorBuilder:");
    let vector = MultivectorBuilder::<3, 0, 0>::new()
        .e(1, 1.0) // x component
        .e(2, 1.0) // y component
        .e(3, 0.5) // z component
        .build();

    println!("Vector v = x·e1 + y·e2 + 0.5·e3");
    println!(
        "v = [{:.1}, {:.1}, {:.1}, {:.1}, {:.1}, {:.1}, {:.1}, {:.1}]",
        vector.get(0),
        vector.get(1),
        vector.get(2),
        vector.get(3),
        vector.get(4),
        vector.get(5),
        vector.get(6),
        vector.get(7)
    );

    let rotated_vector = rotor.apply(&vector);
    println!("Rotated vector R·v·R†:");
    println!(
        "R·v·R† = [{:.3}, {:.3}, {:.3}, {:.3}, {:.3}, {:.3}, {:.3}, {:.3}]",
        rotated_vector.get(0),
        rotated_vector.get(1),
        rotated_vector.get(2),
        rotated_vector.get(3),
        rotated_vector.get(4),
        rotated_vector.get(5),
        rotated_vector.get(6),
        rotated_vector.get(7)
    );

    println!(
        "\nNote: The z-component (e3) should remain unchanged: {:.3} → {:.3}",
        vector.get(4),
        rotated_vector.get(4)
    );

    println!("\nExample completed successfully!");
}
