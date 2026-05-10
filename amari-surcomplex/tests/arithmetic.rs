use amari_surcomplex::RationalSurcomplex;
use amari_surreal::RationalSurreal;

#[test]
fn rational_surcomplex_multiplies_i_squared_to_minus_one() {
    let i = RationalSurcomplex::i();
    assert_eq!(i.clone() * i, RationalSurcomplex::from_integer(-1));
}

#[test]
fn rational_surcomplex_division_produces_non_dyadic_coefficients() {
    let one = RationalSurreal::one();
    let half = RationalSurreal::from_ratio(1, 2).unwrap();
    let z = RationalSurcomplex::from_parts(one.clone(), half);
    let quotient = RationalSurcomplex::one().checked_div(&z).unwrap();
    assert_eq!(quotient.real().to_string(), "4/5");
    assert_eq!(quotient.imag().to_string(), "-2/5");
}

#[test]
fn rational_surcomplex_norm_and_conjugate_identity() {
    let z = RationalSurcomplex::from_parts(
        RationalSurreal::from_ratio(3, 2).unwrap(),
        RationalSurreal::from_ratio(-5, 3).unwrap(),
    );
    let product = z.clone() * z.conjugate();
    assert_eq!(product.imag(), &RationalSurreal::zero());
    assert_eq!(product.real(), &z.norm_sq());
}
