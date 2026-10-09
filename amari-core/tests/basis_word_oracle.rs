// Copyright (C) 2026 Industrial Algebra
// SPDX-License-Identifier: Apache-2.0
use amari_core::{CayleyTable, Multivector};

fn coefficients<const P: usize, const Q: usize, const R: usize>(
    actual: &Multivector<P, Q, R>,
    expected: &Multivector<P, Q, R>,
) {
    for (i, (&a, &b)) in actual
        .as_slice()
        .iter()
        .zip(expected.as_slice())
        .enumerate()
    {
        assert!((a - b).abs() < 1e-10, "blade {i}: actual={a}, expected={b}");
    }
}

fn blade<const P: usize, const Q: usize, const R: usize>(index: usize) -> Multivector<P, Q, R> {
    let mut mv = Multivector::zero();
    mv.set(index, 1.0);
    mv
}

// Independent oracle (promoted from docs/audits/core-foundations-repro/tests/foundations.rs): concatenate basis words, sort via adjacent swaps,
// cancel repeated generators with their metric, then rebuild the blade.
// No XOR product or implementation's bit-pair sign formula is used.
fn word_product(a: usize, b: usize, p: usize, q: usize, r: usize) -> (f64, usize) {
    let n = p + q + r;
    let mut word = Vec::new();
    for mask in [a, b] {
        for i in 0..n {
            if mask & (1 << i) != 0 {
                word.push(i);
            }
        }
    }
    let mut sign = 1.0;
    for end in (1..word.len()).rev() {
        for i in 0..end {
            if word[i] > word[i + 1] {
                word.swap(i, i + 1);
                sign = -sign;
            }
        }
    }
    let mut index = 0;
    let mut i = 0;
    while i < word.len() {
        if i + 1 < word.len() && word[i] == word[i + 1] {
            sign *= if word[i] < p {
                1.0
            } else if word[i] < p + q {
                -1.0
            } else {
                0.0
            };
            i += 2;
        } else {
            index += 1 << word[i];
            i += 1;
        }
    }
    (sign, index)
}

fn check_signature<const P: usize, const Q: usize, const R: usize>() {
    let n = P + Q + R;
    let size = 1 << n;
    let table = CayleyTable::<P, Q, R>::new();
    for a in 0..size {
        let x = blade::<P, Q, R>(a);
        let k = a.count_ones() as usize;
        for b in 0..size {
            let y = blade::<P, Q, R>(b);
            let l = b.count_ones() as usize;
            let (sign, out) = word_product(a, b, P, Q, R);
            assert_eq!(
                table.get_product(a, b),
                (sign, out),
                "Cl({P},{Q},{R}), {a}*{b}"
            );
            let product = blade::<P, Q, R>(out) * sign;
            coefficients(&x.geometric_product(&y), &product);
            let grade = out.count_ones() as usize;
            coefficients(
                &x.outer_product(&y),
                &(product.clone() * if grade == k + l { 1.0 } else { 0.0 }),
            );
            // This validates the implemented grade-difference convention,
            // including scalars; it does not label it the Hestenes convention.
            coefficients(
                &x.inner_product(&y),
                &(product.clone() * if grade == k.abs_diff(l) { 1.0 } else { 0.0 }),
            );
            coefficients(
                &x.left_contraction(&y),
                &(product.clone() * if l >= k && grade == l - k { 1.0 } else { 0.0 }),
            );
            coefficients(
                &x.right_contraction(&y),
                &(product * if k >= l && grade == k - l { 1.0 } else { 0.0 }),
            );
            coefficients(
                &x.geometric_product(&y).reverse(),
                &y.reverse().geometric_product(&x.reverse()),
            );
        }
        if R == 0 {
            let double_star_sign = if (k * (n - k) + Q).is_multiple_of(2) {
                1.0
            } else {
                -1.0
            };
            coefficients(&x.hodge_dual().hodge_dual(), &(x * double_star_sign));
        }
    }
}

#[test]
fn oracle_all_basis_pairs_through_dimension_four() {
    macro_rules! signatures {
        ($(($p:literal, $q:literal, $r:literal)),* $(,)?) => { $(check_signature::<$p, $q, $r>();)* };
    }
    signatures!(
        (0, 0, 0),
        (1, 0, 0),
        (0, 1, 0),
        (0, 0, 1),
        (2, 0, 0),
        (1, 1, 0),
        (1, 0, 1),
        (0, 2, 0),
        (0, 1, 1),
        (0, 0, 2),
        (3, 0, 0),
        (2, 1, 0),
        (2, 0, 1),
        (1, 2, 0),
        (1, 1, 1),
        (1, 0, 2),
        (0, 3, 0),
        (0, 2, 1),
        (0, 1, 2),
        (0, 0, 3),
        (4, 0, 0),
        (3, 1, 0),
        (3, 0, 1),
        (2, 2, 0),
        (2, 1, 1),
        (2, 0, 2),
        (1, 3, 0),
        (1, 2, 1),
        (1, 1, 2),
        (1, 0, 3),
        (0, 4, 0),
        (0, 3, 1),
        (0, 2, 2),
        (0, 1, 3),
        (0, 0, 4),
    );
}
