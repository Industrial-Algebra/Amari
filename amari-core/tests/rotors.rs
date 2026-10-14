use amari_core::{Bivector, Rotor, RotorError, Vector};
use approx::assert_relative_eq;
use std::f64::consts::PI;

mod rotor_tests {
    use super::*;

    #[test]
    fn test_rotor_from_bivector() {
        // Create rotor for 90° rotation in e12 plane
        let bivector = Bivector::<3, 0, 0>::e12().expect("Cl(3,0,0) has blade e12");
        let rotor = Rotor::from_bivector(&bivector, PI / 2.0)
            .expect("bounded Euclidean bivector exponential");

        // Rotor should be normalized
        assert_relative_eq!(
            rotor.as_multivector().coefficient_norm(),
            1.0,
            epsilon = 1e-10
        );
    }

    #[test]
    fn test_rotor_rotation_of_vector() {
        // Rotate e1 by 90° in the e12 plane to get e2
        let bivector = Bivector::<3, 0, 0>::e12().expect("Cl(3,0,0) has blade e12");
        let rotor = Rotor::from_bivector(&bivector, PI / 2.0)
            .expect("bounded Euclidean bivector exponential");
        let e1 = Vector::<3, 0, 0>::e1().expect("Cl(3,0,0) has basis vector e1");

        // Apply rotation: R * v * R†
        let rotated = rotor
            .apply_to_vector(&e1)
            .expect("certified rotor acts on a vector");

        // Should be close to e2
        assert_relative_eq!(
            rotated.as_multivector().vector_component(0),
            0.0,
            epsilon = 1e-10
        ); // e1 component
        assert_relative_eq!(
            rotated.as_multivector().vector_component(1),
            1.0,
            epsilon = 1e-10
        ); // e2 component
        assert_relative_eq!(
            rotated.as_multivector().vector_component(2),
            0.0,
            epsilon = 1e-10
        );
        // e3 component
    }

    #[test]
    fn test_rotor_double_cover() {
        // 2π rotation should give negative of identity (double cover of SO(3))
        let bivector = Bivector::<3, 0, 0>::e12().expect("Cl(3,0,0) has blade e12");
        let rotor_2pi = Rotor::from_bivector(&bivector, 2.0 * PI)
            .expect("bounded Euclidean bivector exponential");

        // Should be -1 in scalar part
        assert_relative_eq!(rotor_2pi.scalar_part(), -1.0, epsilon = 1e-10);
    }

    #[test]
    fn test_rotor_composition() {
        // Two 45° rotations should equal one 90° rotation
        let bivector = Bivector::<3, 0, 0>::e12().expect("Cl(3,0,0) has blade e12");
        let rotor_45 = Rotor::from_bivector(&bivector, PI / 4.0)
            .expect("bounded Euclidean bivector exponential");
        let rotor_90 = Rotor::from_bivector(&bivector, PI / 2.0)
            .expect("bounded Euclidean bivector exponential");

        let composed = rotor_45.geometric_product(&rotor_45);

        assert_relative_eq!(composed.as_slice(), rotor_90.as_slice(), epsilon = 1e-10);
    }

    // ============ Extended Rotor Tests ============

    #[test]
    fn test_rotor_identity() {
        // Identity rotor should be scalar 1
        let identity = Rotor::<3, 0, 0>::identity();

        assert_relative_eq!(identity.scalar_part(), 1.0, epsilon = 1e-10);
        assert_relative_eq!(
            identity.bivector_part().coefficient_norm(),
            0.0,
            epsilon = 1e-10
        );
    }

    #[test]
    fn test_rotor_inverse() {
        // R * R† = 1 for unit rotors
        let bivector = Bivector::<3, 0, 0>::e12().expect("Cl(3,0,0) has blade e12");
        let rotor = Rotor::from_bivector(&bivector, PI / 3.0)
            .expect("bounded Euclidean bivector exponential");
        let inverse = rotor.inverse().expect("certified rotor inverse");

        let product = rotor.geometric_product(&inverse);

        assert_relative_eq!(product.scalar_part(), 1.0, epsilon = 1e-10);
        assert_relative_eq!(
            product.bivector_part().coefficient_norm(),
            0.0,
            epsilon = 1e-10
        );
    }

    #[test]
    fn test_rotor_from_vectors() {
        // Create rotor that rotates from one vector to another
        let from = Vector::<3, 0, 0>::e1().expect("Cl(3,0,0) has basis vector e1");
        let to = Vector::<3, 0, 0>::e2().expect("Cl(3,0,0) has basis vector e2");

        let rotor = Rotor::from_vectors(&from, &to).expect("Should be able to create rotor");
        let rotated = rotor
            .apply_to_vector(&from)
            .expect("certified rotor acts on a vector");

        // Should rotate e1 to e2
        assert_relative_eq!(rotated.as_slice(), to.as_slice(), epsilon = 1e-10);
    }

    #[test]
    fn test_rotor_interpolation() {
        // Test spherical linear interpolation (slerp) between rotors
        let identity = Rotor::<3, 0, 0>::identity();
        let bivector = Bivector::<3, 0, 0>::e12().expect("Cl(3,0,0) has blade e12");
        let target = Rotor::from_bivector(&bivector, PI / 2.0)
            .expect("bounded Euclidean bivector exponential");

        // Interpolate halfway
        let halfway = identity.slerp(&target, 0.5).expect("interior slerp");
        let expected = Rotor::from_bivector(&bivector, PI / 4.0)
            .expect("bounded Euclidean bivector exponential");

        assert_relative_eq!(halfway.as_slice(), expected.as_slice(), epsilon = 1e-10);
    }

    #[test]
    fn test_rotor_to_matrix() {
        // 90° rotation in e12 plane: e1→e2, e2→-e1, e3 unchanged
        let bivector = Bivector::<3, 0, 0>::e12().expect("Cl(3,0,0) has blade e12");
        let rotor = Rotor::from_bivector(&bivector, PI / 2.0)
            .expect("bounded Euclidean bivector exponential");
        let matrix = rotor
            .to_rotation_matrix()
            .expect("in-domain bounded e12 rotor acts");

        // Verify against apply_to_vector
        let e1 = Vector::<3, 0, 0>::e1().expect("Cl(3,0,0) has basis vector e1");
        let e2 = Vector::<3, 0, 0>::e2().expect("Cl(3,0,0) has basis vector e2");
        let e3 = Vector::<3, 0, 0>::e3().expect("Cl(3,0,0) has basis vector e3");

        let r_e1 = rotor
            .apply_to_vector(&e1)
            .expect("certified rotor acts on a vector");
        let r_e2 = rotor
            .apply_to_vector(&e2)
            .expect("certified rotor acts on a vector");
        let r_e3 = rotor
            .apply_to_vector(&e3)
            .expect("certified rotor acts on a vector");

        for (i, row) in matrix.iter().enumerate() {
            assert_relative_eq!(
                row[0],
                r_e1.as_multivector().vector_component(i),
                epsilon = 1e-10
            );
            assert_relative_eq!(
                row[1],
                r_e2.as_multivector().vector_component(i),
                epsilon = 1e-10
            );
            assert_relative_eq!(
                row[2],
                r_e3.as_multivector().vector_component(i),
                epsilon = 1e-10
            );
        }

        // e3 should be unchanged
        assert_relative_eq!(matrix[2][2], 1.0, epsilon = 1e-10);
    }

    #[test]
    fn test_rotor_from_axis_angle() {
        // Create rotor from axis-angle representation
        let axis = Vector::<3, 0, 0>::from_components(0.0, 0.0, 1.0);
        let angle = PI / 2.0;
        let rotor =
            Rotor::from_axis_angle(&axis, angle).expect("bounded Euclidean bivector exponential");

        // Should be equivalent to e12 bivector rotation
        let bivector_rotor = Rotor::from_bivector(
            &Bivector::<3, 0, 0>::e12().expect("Cl(3,0,0) has blade e12"),
            angle,
        )
        .expect("bounded Euclidean bivector exponential");

        assert_relative_eq!(rotor.as_slice(), bivector_rotor.as_slice(), epsilon = 1e-10);
    }

    #[test]
    fn test_rotor_preserves_magnitude() {
        // Rotation should preserve vector magnitudes
        let bivector = Bivector::<3, 0, 0>::e12().expect("Cl(3,0,0) has blade e12");
        let rotor = Rotor::from_bivector(&bivector, PI / 3.0)
            .expect("bounded Euclidean bivector exponential");
        let vector = Vector::<3, 0, 0>::from_components(3.0, 4.0, 5.0);

        let rotated = rotor
            .apply_to_vector(&vector)
            .expect("certified rotor acts on a vector");

        assert_relative_eq!(
            vector.coefficient_norm(),
            rotated.coefficient_norm(),
            epsilon = 1e-10
        );
    }

    #[test]
    fn test_rotor_preserves_angles() {
        // Rotation should preserve angles between vectors
        let bivector = Bivector::<3, 0, 0>::e23().expect("Cl(3,0,0) has blade e23"); // Rotate in yz plane
        let rotor = Rotor::from_bivector(&bivector, PI / 6.0)
            .expect("bounded Euclidean bivector exponential");

        let v1 = Vector::<3, 0, 0>::from_components(1.0, 1.0, 0.0);
        let v2 = Vector::<3, 0, 0>::from_components(1.0, 0.0, 1.0);

        let original_dot = v1.inner_product(&v2).scalar_part();

        let r1 = rotor
            .apply_to_vector(&v1)
            .expect("certified rotor acts on a vector");
        let r2 = rotor
            .apply_to_vector(&v2)
            .expect("certified rotor acts on a vector");
        let rotated_dot = r1.inner_product(&r2).scalar_part();

        assert_relative_eq!(original_dot, rotated_dot, epsilon = 1e-10);
    }

    // ============ Complex Rotation Tests ============

    #[test]
    fn test_euler_angle_composition() {
        // Test composition of Euler angle rotations
        let x_rot = Rotor::from_axis_angle(
            &Vector::<3, 0, 0>::e1().expect("Cl(3,0,0) has basis vector e1"),
            PI / 4.0,
        )
        .expect("bounded Euclidean bivector exponential");
        let y_rot = Rotor::from_axis_angle(
            &Vector::<3, 0, 0>::e2().expect("Cl(3,0,0) has basis vector e2"),
            PI / 3.0,
        )
        .expect("bounded Euclidean bivector exponential");
        let z_rot = Rotor::from_axis_angle(
            &Vector::<3, 0, 0>::e3().expect("Cl(3,0,0) has basis vector e3"),
            PI / 6.0,
        )
        .expect("bounded Euclidean bivector exponential");

        // Compose rotations: first z, then y, then x
        let composed = x_rot.geometric_product(&y_rot.geometric_product(&z_rot));

        // Should still be a unit rotor
        assert_relative_eq!(
            composed.as_multivector().coefficient_norm(),
            1.0,
            epsilon = 1e-10
        );
    }

    #[test]
    fn test_rotor_commutator() {
        // Test commutator of non-commuting rotations
        let rotor_a = Rotor::from_axis_angle(
            &Vector::<3, 0, 0>::e1().expect("Cl(3,0,0) has basis vector e1"),
            PI / 4.0,
        )
        .expect("bounded Euclidean bivector exponential");
        let rotor_b = Rotor::from_axis_angle(
            &Vector::<3, 0, 0>::e2().expect("Cl(3,0,0) has basis vector e2"),
            PI / 4.0,
        )
        .expect("bounded Euclidean bivector exponential");

        let ab = rotor_a.geometric_product(&rotor_b);
        let ba = rotor_b.geometric_product(&rotor_a);

        // Should not commute (AB ≠ BA) - check if any component differs
        let mut differs = false;
        for i in 0..ab.as_slice().len() {
            if (ab.as_slice()[i] - ba.as_slice()[i]).abs() > 1e-10 {
                differs = true;
                break;
            }
        }
        assert!(differs, "AB and BA should not be identical");
    }

    #[test]
    fn test_rotor_logarithm() {
        // Test rotor logarithm: log(exp(X)) = X
        // from_bivector creates R = exp(-B*θ/2), so log(R) = -B*θ/2
        let bivector = Bivector::<3, 0, 0>::e12().expect("Cl(3,0,0) has blade e12");
        let angle = PI / 3.0;
        let rotor =
            Rotor::from_bivector(&bivector, angle).expect("bounded Euclidean bivector exponential");

        let log_rotor = rotor
            .logarithm()
            .expect("bounded Euclidean rotor is a certified log class");
        // The rotor stores exp(-B*angle/2), so log gives back -B*angle/2
        let expected_bivector = bivector.as_multivector() * (-angle / 2.0);

        assert_relative_eq!(
            log_rotor.as_slice(),
            expected_bivector.as_slice(),
            epsilon = 1e-10
        );
    }

    #[test]
    fn test_rotor_power() {
        // Test raising rotor to a power
        let bivector = Bivector::<3, 0, 0>::e12().expect("Cl(3,0,0) has blade e12");
        let rotor = Rotor::from_bivector(&bivector, PI / 4.0)
            .expect("bounded Euclidean bivector exponential");

        let squared = rotor.power(2.0).expect("bounded rotor power exponential");
        let manual_square = rotor.geometric_product(&rotor);

        assert_relative_eq!(
            squared.as_slice(),
            manual_square.as_slice(),
            epsilon = 1e-10
        );
    }

    // ============ Audit: Additional Rotor Tests ============

    #[test]
    fn test_rotor_power_fractional() {
        // R^0.5 of a 90° rotation should give a 45° rotation
        let bivector = Bivector::<3, 0, 0>::e12().expect("Cl(3,0,0) has blade e12");
        let rotor_90 = Rotor::from_bivector(&bivector, PI / 2.0)
            .expect("bounded Euclidean bivector exponential");
        let rotor_45 = Rotor::from_bivector(&bivector, PI / 4.0)
            .expect("bounded Euclidean bivector exponential");

        let half_power = rotor_90
            .power(0.5)
            .expect("bounded rotor power exponential");

        assert_relative_eq!(half_power.as_slice(), rotor_45.as_slice(), epsilon = 1e-10);
    }

    #[test]
    fn test_rotor_power_zero() {
        // R^0 should give identity
        let bivector = Bivector::<3, 0, 0>::e12().expect("Cl(3,0,0) has blade e12");
        let rotor = Rotor::from_bivector(&bivector, PI / 3.0)
            .expect("bounded Euclidean bivector exponential");
        let identity = Rotor::<3, 0, 0>::identity();

        let power_zero = rotor.power(0.0).expect("bounded rotor power exponential");

        assert_relative_eq!(power_zero.as_slice(), identity.as_slice(), epsilon = 1e-10);
    }

    #[test]
    fn test_rotor_power_negative() {
        // R^-1 should give the inverse
        let bivector = Bivector::<3, 0, 0>::e12().expect("Cl(3,0,0) has blade e12");
        let rotor = Rotor::from_bivector(&bivector, PI / 3.0)
            .expect("bounded Euclidean bivector exponential");
        let inverse = rotor.inverse().expect("certified rotor inverse");

        let power_neg1 = rotor.power(-1.0).expect("bounded rotor power exponential");

        assert_relative_eq!(power_neg1.as_slice(), inverse.as_slice(), epsilon = 1e-10);
    }

    #[test]
    fn test_rotor_power_third() {
        // R^3 = R * R * R
        let bivector = Bivector::<3, 0, 0>::e12().expect("Cl(3,0,0) has blade e12");
        let rotor = Rotor::from_bivector(&bivector, PI / 6.0)
            .expect("bounded Euclidean bivector exponential");

        let cubed = rotor.power(3.0).expect("bounded rotor power exponential");
        let manual_cube = rotor.geometric_product(&rotor).geometric_product(&rotor);

        assert_relative_eq!(cubed.as_slice(), manual_cube.as_slice(), epsilon = 1e-10);
    }

    #[test]
    fn test_rotor_rotation_e13_plane() {
        // Rotate e1 by 90° in the e13 plane → should get e3
        let bivector = Bivector::<3, 0, 0>::e13().expect("Cl(3,0,0) has blade e13");
        let rotor = Rotor::from_bivector(&bivector, PI / 2.0)
            .expect("bounded Euclidean bivector exponential");
        let e1 = Vector::<3, 0, 0>::e1().expect("Cl(3,0,0) has basis vector e1");

        let rotated = rotor
            .apply_to_vector(&e1)
            .expect("certified rotor acts on a vector");

        assert_relative_eq!(
            rotated.as_multivector().vector_component(0),
            0.0,
            epsilon = 1e-10
        ); // e1
        assert_relative_eq!(
            rotated.as_multivector().vector_component(1),
            0.0,
            epsilon = 1e-10
        ); // e2
        assert_relative_eq!(
            rotated.as_multivector().vector_component(2),
            1.0,
            epsilon = 1e-10
        );
        // e3
    }

    #[test]
    fn test_rotor_rotation_e23_plane() {
        // Rotate e2 by 90° in the e23 plane → should get e3
        let bivector = Bivector::<3, 0, 0>::e23().expect("Cl(3,0,0) has blade e23");
        let rotor = Rotor::from_bivector(&bivector, PI / 2.0)
            .expect("bounded Euclidean bivector exponential");
        let e2 = Vector::<3, 0, 0>::e2().expect("Cl(3,0,0) has basis vector e2");

        let rotated = rotor
            .apply_to_vector(&e2)
            .expect("certified rotor acts on a vector");

        assert_relative_eq!(
            rotated.as_multivector().vector_component(0),
            0.0,
            epsilon = 1e-10
        ); // e1
        assert_relative_eq!(
            rotated.as_multivector().vector_component(1),
            0.0,
            epsilon = 1e-10
        ); // e2
        assert_relative_eq!(
            rotated.as_multivector().vector_component(2),
            1.0,
            epsilon = 1e-10
        );
        // e3
    }

    #[test]
    fn test_rotor_180_degree() {
        // 180° rotation of e1 in e12 plane → should get -e1
        let bivector = Bivector::<3, 0, 0>::e12().expect("Cl(3,0,0) has blade e12");
        let rotor =
            Rotor::from_bivector(&bivector, PI).expect("bounded Euclidean bivector exponential");
        let e1 = Vector::<3, 0, 0>::e1().expect("Cl(3,0,0) has basis vector e1");

        let rotated = rotor
            .apply_to_vector(&e1)
            .expect("certified rotor acts on a vector");

        assert_relative_eq!(
            rotated.as_multivector().vector_component(0),
            -1.0,
            epsilon = 1e-10
        ); // e1
        assert_relative_eq!(
            rotated.as_multivector().vector_component(1),
            0.0,
            epsilon = 1e-10
        );
        // e2
    }

    #[test]
    fn test_rotor_small_angle() {
        // Very small rotation should be near identity
        let bivector = Bivector::<3, 0, 0>::e12().expect("Cl(3,0,0) has blade e12");
        let rotor =
            Rotor::from_bivector(&bivector, 1e-10).expect("bounded Euclidean bivector exponential");
        let e1 = Vector::<3, 0, 0>::e1().expect("Cl(3,0,0) has basis vector e1");

        let rotated = rotor
            .apply_to_vector(&e1)
            .expect("certified rotor acts on a vector");

        assert_relative_eq!(
            rotated.as_multivector().vector_component(0),
            1.0,
            epsilon = 1e-8
        );
        assert_relative_eq!(
            rotated.as_multivector().vector_component(1),
            0.0,
            epsilon = 1e-8
        );
    }

    #[test]
    fn test_to_rotation_matrix_e13_plane() {
        // Verify matrix matches apply() for e13 rotation
        let bivector = Bivector::<3, 0, 0>::e13().expect("Cl(3,0,0) has blade e13");
        let rotor = Rotor::from_bivector(&bivector, PI / 2.0)
            .expect("bounded Euclidean bivector exponential");
        let matrix = rotor
            .to_rotation_matrix()
            .expect("in-domain bounded e13 rotor acts");

        // Compute expected via apply
        let e1 = Vector::<3, 0, 0>::e1().expect("Cl(3,0,0) has basis vector e1");
        let e2 = Vector::<3, 0, 0>::e2().expect("Cl(3,0,0) has basis vector e2");
        let e3 = Vector::<3, 0, 0>::e3().expect("Cl(3,0,0) has basis vector e3");

        let r_e1 = rotor
            .apply_to_vector(&e1)
            .expect("certified rotor acts on a vector");
        let r_e2 = rotor
            .apply_to_vector(&e2)
            .expect("certified rotor acts on a vector");
        let r_e3 = rotor
            .apply_to_vector(&e3)
            .expect("certified rotor acts on a vector");

        // Matrix columns should match rotated basis vectors
        for (i, row) in matrix.iter().enumerate() {
            assert_relative_eq!(
                row[0],
                r_e1.as_multivector().vector_component(i),
                epsilon = 1e-10
            );
            assert_relative_eq!(
                row[1],
                r_e2.as_multivector().vector_component(i),
                epsilon = 1e-10
            );
            assert_relative_eq!(
                row[2],
                r_e3.as_multivector().vector_component(i),
                epsilon = 1e-10
            );
        }

        // e2 should be unchanged by e13 rotation
        assert_relative_eq!(matrix[1][1], 1.0, epsilon = 1e-10);
    }

    #[test]
    fn test_rotor_from_axis_angle_diagonal() {
        // Rotation around (1,1,1)/sqrt(3) axis
        let axis = Vector::<3, 0, 0>::from_components(1.0, 1.0, 1.0);
        let angle = 2.0 * PI / 3.0; // 120°
        let rotor =
            Rotor::from_axis_angle(&axis, angle).expect("bounded Euclidean bivector exponential");

        // 120° rotation around (1,1,1) cycles e1→e2→e3→e1
        let e1 = Vector::<3, 0, 0>::e1().expect("Cl(3,0,0) has basis vector e1");
        let rotated = rotor
            .apply_to_vector(&e1)
            .expect("certified rotor acts on a vector");

        assert_relative_eq!(
            rotated.as_multivector().vector_component(0),
            0.0,
            epsilon = 1e-10
        ); // e1
        assert_relative_eq!(
            rotated.as_multivector().vector_component(1),
            1.0,
            epsilon = 1e-10
        ); // e2
        assert_relative_eq!(
            rotated.as_multivector().vector_component(2),
            0.0,
            epsilon = 1e-10
        );
        // e3
    }

    #[test]
    fn test_rotor_from_axis_angle_is_range_safe() {
        // R1-F7: `|axis| ~ 1e200` overflows the old `norm_squared()` (spurious
        // identity) and `|axis| ~ 1e-200` underflows it to 0 (spurious
        // NullVector). The exact scalar square normalizes before any
        // null/finite decision, so both reduce to the e1 rotation.
        let e1 = Vector::<3, 0, 0>::e1().expect("Cl(3,0,0) has basis vector e1");
        let base = Rotor::from_axis_angle(&e1, 1.0).expect("baseline e1 rotation");
        assert_eq!(base.as_slice()[0], 0.8775825618903728);
        assert_eq!(base.as_slice()[6], -0.479425538604203);

        let huge = Vector::<3, 0, 0>::from_components(1e200, 0.0, 0.0);
        let huge_rotor = Rotor::from_axis_angle(&huge, 1.0).expect("range-safe huge axis");
        assert_eq!(huge_rotor.as_slice(), base.as_slice());

        let tiny = Vector::<3, 0, 0>::from_components(1e-200, 0.0, 0.0);
        let tiny_rotor = Rotor::from_axis_angle(&tiny, 1.0).expect("range-safe tiny axis");
        assert_eq!(tiny_rotor.as_slice(), base.as_slice());

        let nan = Vector::<3, 0, 0>::from_components(f64::NAN, 0.0, 0.0);
        assert!(matches!(
            Rotor::from_axis_angle(&nan, 1.0),
            Err(RotorError::NotNormalizable)
        ));
    }

    #[test]
    fn test_rotor_from_axis_angle_normalizes_overflowing_axes_symbolically() {
        // R2-F10: `[MAX, MAX, 0]` certifies an exact sigma whose square root
        // overflows f64; the symbolic normalization must produce the same
        // rotation as `[1, 1, 0]` instead of silently returning identity.
        let base = Rotor::from_axis_angle(&Vector::<3, 0, 0>::from_components(1.0, 1.0, 0.0), 1.0)
            .expect("baseline [1,1,0] rotation");
        assert_eq!(base.as_slice()[0], 0.8775825618903728);
        assert_eq!(base.as_slice()[5], 0.3390050494210448);
        assert_eq!(base.as_slice()[6], -0.3390050494210448);

        let huge = Vector::<3, 0, 0>::from_components(f64::MAX, f64::MAX, 0.0);
        let huge_rotor = Rotor::from_axis_angle(&huge, 1.0).expect("range-safe overflowing axis");
        assert_eq!(huge_rotor.as_slice(), base.as_slice());

        // The single-blade extreme stays bit-exact against the e1 rotation.
        let e1 = Vector::<3, 0, 0>::e1().expect("Cl(3,0,0) has basis vector e1");
        let e1_base = Rotor::from_axis_angle(&e1, 1.0).expect("baseline e1 rotation");
        let huge_e1 = Vector::<3, 0, 0>::from_components(f64::MAX, 0.0, 0.0);
        let huge_e1_rotor =
            Rotor::from_axis_angle(&huge_e1, 1.0).expect("range-safe single-blade axis");
        assert_eq!(huge_e1_rotor.as_slice(), e1_base.as_slice());

        // Mixed-magnitude axes stay green.
        let mixed = Vector::<3, 0, 0>::from_components(1e200, 1e-200, 1.0);
        let mixed_rotor = Rotor::from_axis_angle(&mixed, 1.0).expect("mixed-magnitude axis");
        assert!(mixed_rotor.as_slice().iter().all(|c| c.is_finite()));
        assert_eq!(mixed_rotor.as_slice()[0], 0.8775825618903728);
        assert_eq!(mixed_rotor.as_slice()[6], -0.479425538604203);
    }

    #[test]
    fn test_slerp_endpoints() {
        let bivector = Bivector::<3, 0, 0>::e12().expect("Cl(3,0,0) has blade e12");
        let r1 = Rotor::from_bivector(&bivector, PI / 6.0)
            .expect("bounded Euclidean bivector exponential");
        let r2 = Rotor::from_bivector(&bivector, PI / 2.0)
            .expect("bounded Euclidean bivector exponential");

        // t=0 should give r1
        let at_0 = r1.slerp(&r2, 0.0).expect("endpoint t = 0");
        assert_relative_eq!(at_0.as_slice(), r1.as_slice(), epsilon = 1e-10);

        // t=1 should give r2
        let at_1 = r1.slerp(&r2, 1.0).expect("endpoint t = 1");
        assert_relative_eq!(at_1.as_slice(), r2.as_slice(), epsilon = 1e-10);
    }
}
