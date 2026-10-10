//! Unicode mathematical operations for Amari
//!
//! This module provides Unicode-inspired macro aliases for geometric algebra operations,
//! making mathematical expressions more readable and intuitive while preserving all existing APIs.
//!
//! # Supported Unicode-Inspired Operators
//!
//! ## Binary Operations
//! - `geo!(a, b)`: Geometric product (⊗ - tensor product symbol)
//! - `wedge!(a, b)`: Wedge/outer product (∧ - logical AND symbol)  
//! - `dot!(a, b)`: Inner product (• - bullet operator)
//! - `lcon!(a, b)`: Left contraction (⌟ - bottom left corner)
//! - `rcon!(a, b)`: Right contraction (⌞ - bottom right corner)
//!
//! ## Unary Operations
//! - `dual!(a)`: Hodge dual (⋆ - star operator)
//! - `rev!(a)`: Reverse/conjugate († - dagger)
//! - `norm!(a)`: Magnitude/norm (‖·‖ - double vertical bars)
//!
//! ## Grade Operations
//! - `grade!(a, k)`: Grade projection (⟨·⟩ₖ - angle brackets with subscript)
//!
//! # Examples
//!
//! ```rust
//! use amari_core::{Multivector, Vector, Bivector};
//! use amari_core::{geo, wedge, dot, rev, dual, norm};
//!
//! let a = Vector::<3, 0, 0>::e1();
//! let b = Vector::<3, 0, 0>::e2();
//!
//! // Traditional syntax
//! let geometric = a.geometric_product(&b);
//! let wedge_traditional = a.outer_product(&b);
//! let inner = a.inner_product(&b);
//!
//! // Unicode-inspired DSL syntax
//! let geometric2 = geo!(a, b);      // ⊗
//! let wedge2 = wedge!(a, b);        // ∧
//! let inner2 = dot!(a, b);          // •
//!
//! // Unary operations
//! let reversed = rev!(a);          // †
//! let hodge_dual = dual!(a);       // ⋆
//! let magnitude = norm!(a);        // ‖·‖
//! ```

/// Geometric product: a ⊗ b
///
/// The fundamental product in geometric algebra that combines
/// both inner and outer products. Also known as the Clifford product.
#[macro_export]
macro_rules! geo {
    ($a:expr, $b:expr) => {
        $a.geometric_product(&$b)
    };
}

/// Wedge/outer product: a ∧ b
///
/// Creates higher-grade elements from lower-grade ones.
/// Anticommutative: a ∧ b = -(b ∧ a)
#[macro_export]
macro_rules! wedge {
    ($a:expr, $b:expr) => {
        $a.outer_product(&$b)
    };
}

/// Inner product: a • b
///
/// Symmetric product that reduces grade.
/// Commutative for vectors: a • b = b • a
#[macro_export]
macro_rules! dot {
    ($a:expr, $b:expr) => {
        $a.inner_product(&$b)
    };
}

/// Left contraction: a ⌟ b
///
/// Generalized inner product where the grade of the result
/// is |grade(b) - grade(a)|
#[macro_export]
macro_rules! lcon {
    ($a:expr, $b:expr) => {
        $a.left_contraction(&$b)
    };
}

/// Right contraction: a ⌞ b  
///
/// Generalized inner product where the grade of the result
/// is |grade(a) - grade(b)|
#[macro_export]
macro_rules! rcon {
    ($a:expr, $b:expr) => {
        $a.right_contraction(&$b)
    };
}

/// Hodge dual: ⋆a
///
/// Maps k-vectors to (n-k)-vectors in n-dimensional space.
/// Essential for electromagnetic field theory and differential forms.
#[macro_export]
macro_rules! dual {
    ($a:expr) => {
        $a.hodge_dual()
    };
}

/// Reverse/conjugate: a†
///
/// Reverses the order of basis vectors in each term.
/// Important for defining magnitudes and inverses.
#[macro_export]
macro_rules! rev {
    ($a:expr) => {
        $a.reverse()
    };
}

/// Grade projection: ⟨a⟩ₖ
///
/// Extracts the k-grade part of a multivector.
/// Grade 0 = scalar, 1 = vector, 2 = bivector, etc.
#[macro_export]
macro_rules! grade {
    ($a:expr, $k:expr) => {
        $a.grade_projection($k)
    };
}

/// Magnitude/norm: ‖a‖
///
/// Coefficient magnitude of the multivector.
/// Computed as sqrt of the sum of squared coefficients.
#[macro_export]
macro_rules! norm {
    ($a:expr) => {
        $a.coefficient_norm()
    };
}

/// Tropical addition (max): a ⊕ b
#[macro_export]
macro_rules! trop_add {
    ($a:expr, $b:expr) => {
        $a.tropical_add(&$b)
    };
}

/// Tropical multiplication (addition): a ⊙ b  
#[macro_export]
macro_rules! trop_mul {
    ($a:expr, $b:expr) => {
        $a.tropical_mul(&$b)
    };
}

/// Commutator: [a, b] = (a⊗b - b⊗a)/2
#[macro_export]
macro_rules! commutator {
    ($a:expr, $b:expr) => {{
        let ab = geo!($a, $b);
        let ba = geo!($b, $a);
        (ab - ba) * 0.5
    }};
}

/// Anticommutator: {a, b} = (a⊗b + b⊗a)/2  
#[macro_export]
macro_rules! anticommutator {
    ($a:expr, $b:expr) => {{
        let ab = geo!($a, $b);
        let ba = geo!($b, $a);
        (ab + ba) * 0.5
    }};
}

/// Squared magnitude: ‖a‖²
#[macro_export]
macro_rules! norm_squared {
    ($a:expr) => {
        $a.norm_squared()
    };
}

/// Unit vector: â = a/‖a‖
#[macro_export]
macro_rules! unit {
    ($a:expr) => {{
        // Public-API expansion (W02 review round 1): the operand contract is
        // any type with `coefficient_norm()` and `Mul<f64>` (Multivector).
        // The previously supported Vector operand is removed with
        // Multivector::normalize (D01.4); convert via `.mv` first.
        {
            let n = $a.coefficient_norm();
            if n > 0.0 {
                $a / n
            } else {
                $a.clone()
            }
        }
    }};
}

#[cfg(test)]
mod tests {
    use crate::{basis::MultivectorBuilder, Vector};
    use approx::assert_relative_eq;

    #[test]
    fn test_unicode_geometric_product() {
        let a = Vector::<3, 0, 0>::e1();
        let b = Vector::<3, 0, 0>::e2();

        // Unicode-inspired syntax
        let unicode_result = geo!(a, b);

        // Traditional syntax
        let traditional_result = a.geometric_product(&b);

        // Should be identical
        for i in 0..8 {
            assert_relative_eq!(unicode_result.get(i), traditional_result.get(i));
        }
    }

    #[test]
    fn test_unicode_wedge_product() {
        let e1 = Vector::<3, 0, 0>::e1();
        let e2 = Vector::<3, 0, 0>::e2();

        let wedge_result = wedge!(e1, e2);
        let traditional = e1.outer_product(&e2);

        // Should produce e12 bivector
        assert_relative_eq!(wedge_result.bivector_part().coefficient_norm(), 1.0);

        // Should match traditional syntax
        for i in 0..8 {
            assert_relative_eq!(wedge_result.get(i), traditional.get(i));
        }
    }

    #[test]
    fn test_unicode_inner_product() {
        let a = Vector::<3, 0, 0>::from_components(3.0, 4.0, 0.0);
        let b = Vector::<3, 0, 0>::from_components(1.0, 1.0, 0.0);

        let inner_result = dot!(a, b);
        let traditional = a.inner_product(&b);

        // Should be scalar: 3*1 + 4*1 = 7
        assert_relative_eq!(inner_result.scalar_part(), 7.0);

        // Should match traditional
        for i in 0..8 {
            assert_relative_eq!(inner_result.get(i), traditional.get(i));
        }
    }

    #[test]
    fn test_unicode_unary_operations() {
        let mv = MultivectorBuilder::<3, 0, 0>::new()
            .scalar(1.0)
            .e(1, 2.0)
            .e(3, 3.0) // e12
            .build();

        // Test reverse
        let reversed = rev!(mv);
        let traditional_reverse = mv.reverse();

        for i in 0..8 {
            assert_relative_eq!(reversed.get(i), traditional_reverse.get(i));
        }

        // Test magnitude
        let magnitude = norm!(mv);
        let traditional_magnitude = mv.coefficient_norm();

        assert_relative_eq!(magnitude, traditional_magnitude);
    }

    #[test]
    fn test_unicode_grade_projection() {
        let mv = MultivectorBuilder::<3, 0, 0>::new()
            .scalar(1.0)
            .e(1, 2.0)
            .e(2, 3.0)
            .e(3, 4.0) // e12
            .build();

        // Test grade projections
        let grade0 = grade!(mv, 0);
        let grade1 = grade!(mv, 1);
        let grade2 = grade!(mv, 2);

        let traditional_grade0 = mv.grade_projection(0);
        let traditional_grade1 = mv.grade_projection(1);
        let traditional_grade2 = mv.grade_projection(2);

        // Should match traditional syntax
        for i in 0..8 {
            assert_relative_eq!(grade0.get(i), traditional_grade0.get(i));
            assert_relative_eq!(grade1.get(i), traditional_grade1.get(i));
            assert_relative_eq!(grade2.get(i), traditional_grade2.get(i));
        }
    }

    #[test]
    fn test_extended_unicode_operations() {
        let a = Vector::<3, 0, 0>::e1();
        let b = Vector::<3, 0, 0>::e2();

        // Test commutator [a,b]
        let commutator_result = commutator!(a, b);

        // For orthogonal vectors, [e1,e2] should be related to e12
        assert!(commutator_result.bivector_part().coefficient_norm() > 0.0);

        // Test unit vector
        let v = Vector::<3, 0, 0>::from_components(3.0, 4.0, 0.0);
        // `unit!` now routes through the crate-private versor normalizer, which
        // is defined on `Multivector`; normalize the multivector form here.
        let unit_result = unit!(v.mv.clone());

        // Should be unit magnitude
        assert_relative_eq!(unit_result.coefficient_norm(), 1.0, epsilon = 1e-10);

        // Test squared magnitude
        let squared_mag = norm_squared!(v);
        let expected = 3.0 * 3.0 + 4.0 * 4.0;
        assert_relative_eq!(squared_mag, expected);
    }

    #[test]
    fn test_unicode_mathematical_identities() {
        let a = Vector::<3, 0, 0>::from_components(1.0, 2.0, 3.0);
        let b = Vector::<3, 0, 0>::from_components(4.0, 5.0, 6.0);

        // Test geometric product decomposition: a⊗b = a•b + a∧b
        let geometric = geo!(a, b);
        let inner = dot!(a, b);
        let outer = wedge!(a, b);
        let decomposed = inner + outer;

        for i in 0..8 {
            assert_relative_eq!(geometric.get(i), decomposed.get(i), epsilon = 1e-10);
        }

        // Test reverse property: (a⊗b)† = b†⊗a†
        let left_side = rev!(geometric);
        let right_side = geo!(rev!(b), rev!(a));

        for i in 0..8 {
            assert_relative_eq!(left_side.get(i), right_side.get(i), epsilon = 1e-10);
        }
    }
}
