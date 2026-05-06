use amari_tropical::{CnfTerm, OrdinalArena, OrdinalWeight, TropicalNumber};
use core::cmp::Ordering;

#[test]
fn max_plus_semiring_laws_hold_on_small_sample() {
    let values = [
        TropicalNumber::neg_infinity(),
        TropicalNumber::new(0.0),
        TropicalNumber::new(1.0),
        TropicalNumber::new(3.0),
    ];

    for &a in &values {
        assert_eq!(a.tropical_add(&TropicalNumber::zero()), a);
        assert_eq!(a.tropical_mul(&TropicalNumber::one()), a);
        assert_eq!(a.tropical_add(&a), a);

        for &b in &values {
            assert_eq!(a.tropical_add(&b), b.tropical_add(&a));
            assert_eq!(a.tropical_mul(&b), b.tropical_mul(&a));

            for &c in &values {
                assert_eq!(
                    a.tropical_add(&b).tropical_add(&c),
                    a.tropical_add(&b.tropical_add(&c))
                );
                assert_eq!(
                    a.tropical_mul(&b).tropical_mul(&c),
                    a.tropical_mul(&b.tropical_mul(&c))
                );
                assert_eq!(
                    a.tropical_mul(&b.tropical_add(&c)),
                    a.tropical_mul(&b).tropical_add(&a.tropical_mul(&c))
                );
            }
        }
    }
}

#[test]
fn ordinal_weight_selection_matches_repeated_oplus() {
    let mut arena = OrdinalArena::new();
    let one = arena.one();
    let omega = arena.omega();
    let omega_plus_one = arena.add(omega, one).unwrap();
    let omega_squared = arena.intern_cnf(vec![CnfTerm::new(omega, 1)]).unwrap();

    let weights = [
        OrdinalWeight::bottom(),
        OrdinalWeight::from_ordinal(one),
        OrdinalWeight::from_ordinal(omega_plus_one),
        OrdinalWeight::from_ordinal(omega_squared),
        OrdinalWeight::from_ordinal(omega),
    ];

    let folded = weights
        .iter()
        .copied()
        .try_fold(OrdinalWeight::bottom(), |acc, weight| {
            acc.oplus(weight, &arena)
        });
    let best = arena.best_weight(&weights).unwrap();

    assert_eq!(folded.unwrap(), best);
    assert_eq!(arena.format_weight(best).unwrap(), "ω^ω");
}

#[test]
fn ordinal_weight_composition_matches_repeated_addition() {
    let mut arena = OrdinalArena::new();
    let one = arena.one();
    let omega = arena.omega();
    let omega_plus_one = arena.add(omega, one).unwrap();

    let weights = [
        OrdinalWeight::from_ordinal(omega),
        OrdinalWeight::from_ordinal(one),
        OrdinalWeight::from_ordinal(one),
    ];

    let composed = arena.compose_weights(&weights).unwrap();
    let expected = OrdinalWeight::from_ordinal(arena.add(omega_plus_one, one).unwrap());

    assert_eq!(composed, expected);
    assert_eq!(arena.format_weight(composed).unwrap(), "ω + 2");
}

#[test]
fn ordinal_weight_semiring_style_associativity_holds_on_small_examples() {
    let mut arena = OrdinalArena::new();
    let zero = OrdinalWeight::one();
    let one = OrdinalWeight::from_ordinal(arena.one());
    let omega = OrdinalWeight::from_ordinal(arena.omega());
    let omega_plus_one = omega.otimes(one, &mut arena).unwrap();
    let samples = [OrdinalWeight::bottom(), zero, one, omega, omega_plus_one];

    for &a in &samples {
        assert_eq!(a.oplus(OrdinalWeight::bottom(), &arena).unwrap(), a);
        assert_eq!(OrdinalWeight::bottom().oplus(a, &arena).unwrap(), a);
        assert_eq!(a.otimes(OrdinalWeight::one(), &mut arena).unwrap(), a);
        assert_eq!(OrdinalWeight::one().otimes(a, &mut arena).unwrap(), a);

        for &b in &samples {
            for &c in &samples {
                let left_add = a.oplus(b, &arena).unwrap().oplus(c, &arena).unwrap();
                let right_add = a.oplus(b.oplus(c, &arena).unwrap(), &arena).unwrap();
                assert_eq!(left_add, right_add);

                let left_mul = a
                    .otimes(b, &mut arena)
                    .unwrap()
                    .otimes(c, &mut arena)
                    .unwrap();
                let right_mul = a
                    .otimes(b.otimes(c, &mut arena).unwrap(), &mut arena)
                    .unwrap();
                assert_eq!(left_mul, right_mul);
            }
        }
    }
}

#[test]
fn ordinal_weight_comparison_orders_bottom_below_ordinals() {
    let mut arena = OrdinalArena::new();
    let one = OrdinalWeight::from_ordinal(arena.one());
    let omega = OrdinalWeight::from_ordinal(arena.omega());

    assert_eq!(
        arena.compare_weight(OrdinalWeight::bottom(), one).unwrap(),
        Ordering::Less
    );
    assert_eq!(arena.compare_weight(one, omega).unwrap(), Ordering::Less);
    assert_eq!(
        arena
            .compare_weight(omega, OrdinalWeight::bottom())
            .unwrap(),
        Ordering::Greater
    );
}
