use amari_core::Multivector;
use amari_holographic::{BindingAlgebra, Cl3, ProductCliffordAlgebra};
const MASK: [usize; 8] = [0, 1, 2, 4, 3, 5, 6, 7];
fn blade(i: usize) -> Cl3 {
    let mut x = Cl3::zero();
    x.set_coeff(i, 1.);
    x
}
fn lift(x: Cl3) -> Multivector<3, 0, 0> {
    let mut m = Multivector::zero();
    for (i, k) in MASK.iter().enumerate() {
        m.set(*k, x.coeff(i));
    }
    m
}
// Inversions in the concatenated sorted words. No shipping sign/table helper.
fn word(slots: &[usize]) -> Cl3 {
    let letters: Vec<_> = slots
        .iter()
        .flat_map(|&i| (0..3).filter(move |k| MASK[i] & (1 << k) != 0))
        .collect();
    let mut sign = 1.;
    let mut mask = 0;
    for (p, &x) in letters.iter().enumerate() {
        mask ^= 1 << x;
        for &y in &letters[p + 1..] {
            if x > y {
                sign = -sign;
            }
        }
    }
    blade(MASK.iter().position(|&k| k == mask).unwrap()).scale(sign)
}
#[test]
fn u2_inverses() {
    let one = blade(0);
    let v = blade(1);
    let x = one.scale(2.) + v;
    assert!((one + v).inverse().is_none());
    assert!(ProductCliffordAlgebra::new([one, one + v])
        .unbind(&ProductCliffordAlgebra::identity())
        .is_err());
    let y = x.inverse().unwrap();
    for (got, expected) in y
        .coefficients()
        .iter()
        .zip(((one.scale(2.) - v).scale(1. / 3.)).coefficients())
    {
        assert!((got - expected).abs() <= 1e-12);
    }
    for val in [x, blade(5), Cl3::scalar(1e-20), Cl3::scalar(1e200)] {
        let inv = val.inverse().unwrap();
        for residual in [val * inv - one, inv * val - one] {
            assert!(residual.coefficients().iter().all(|r| r.abs() <= 1e-12));
        }
    }
    for f in [0., f64::NAN, f64::INFINITY] {
        assert!(Cl3::scalar(f).inverse().is_none());
    }
}

#[test]
fn u1_signed_words() {
    for i in 0..8 {
        for j in 0..8 {
            let (a, b) = (blade(i), blade(j));
            let w = word(&[i, j]);
            assert_eq!(lift(w), lift(a).geometric_product(&lift(b)));
            assert_eq!(a.bind(&b), w);
            for k in 0..8 {
                let c = blade(k);
                let v = word(&[i, j, k]);
                assert_eq!((a * b) * c, v);
                assert_eq!(a * (b * c), v);
            }
            let p = ProductCliffordAlgebra::new([a, b, Cl3::zero()]);
            let q = ProductCliffordAlgebra::new([b, a, b]);
            let expect = [w, word(&[j, i]), Cl3::zero()];
            assert_eq!(*p.bind(&q).components(), expect);
            #[cfg(feature = "parallel")]
            assert_eq!(*p.par_component_product(&q).components(), expect);
        }
    }
    let a = blade(1) + blade(4);
    let b = blade(3) - blade(6);
    assert_eq!(
        a * b,
        word(&[1, 3]) - word(&[1, 6]) + word(&[4, 3]) - word(&[4, 6])
    );
}
