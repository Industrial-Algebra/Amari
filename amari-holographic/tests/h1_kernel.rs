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
fn u3_grade_products() {
    use amari_holographic::algebra::GeometricAlgebra;
    for i in 0..8 {
        for j in 0..8 {
            let (a, b) = (blade(i), blade(j));
            let (r, s) = (MASK[i].count_ones(), MASK[j].count_ones());
            let t = (MASK[i] ^ MASK[j]).count_ones();
            let w = word(&[i, j]);
            for (got, grade, core) in [
                (a.outer_product(&b), r + s, lift(a).outer_product(&lift(b))),
                (
                    a.inner_product(&b),
                    r.abs_diff(s),
                    lift(a).inner_product(&lift(b)),
                ),
            ] {
                assert_eq!(got, if grade == t { w } else { Cl3::zero() });
                assert_eq!(lift(got), core);
            }
        }
    }
    let a = blade(0) + blade(1);
    let b = blade(1) + blade(7);
    assert_eq!(lift(a.inner_product(&b)), lift(a).inner_product(&lift(b)));
    assert_eq!(lift(a.outer_product(&b)), lift(a).outer_product(&lift(b)));
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
fn u4_duals() {
    use amari_holographic::algebra::{CliffordAlgebra as C, GeometricAlgebra};
    let dest = [7, 6, 5, 4, 3, 2, 1, 0];
    let orientation = [1., 1., -1., 1., 1., -1., 1., 1.];
    for i in 0..8 {
        let x = blade(i);
        let c = C::new(lift(x));
        let h = blade(dest[i]).scale(orientation[i]);
        let right = word(&[i, 7]).scale(-1.);
        assert_eq!(x.formal_complement(), h);
        assert_eq!(x.hodge_dual().unwrap(), h);
        assert_eq!(x.right_clifford_dual().unwrap(), right);
        assert_eq!(c.formal_complement().inner(), &lift(h));
        assert_eq!(c.hodge_dual().unwrap().inner(), &lift(h));
        assert_eq!(c.right_clifford_dual().unwrap().inner(), &lift(right));
        let rev = if i >= 4 { -1. } else { 1. };
        assert_eq!(x.reverse(), x.scale(rev));
        assert_eq!(
            x.conjugate(),
            x.scale(if i == 0 || i == 7 { 1. } else { -1. })
        );
    }
    let neg = C::<0, 1, 0>::unit_vector(0).unwrap();
    assert_eq!(neg.hodge_dual().unwrap().get(0).unwrap(), -1.);
    assert_eq!(neg.formal_complement().get(0).unwrap(), 1.);
    let null = C::<0, 0, 1>::unit_vector(0).unwrap();
    for x in [null.clone(), C::identity()] {
        assert!(x.right_clifford_dual().is_err());
        assert!(x.hodge_dual().is_err());
    }
    assert_eq!(null.formal_complement().get(0).unwrap(), 1.);
}
#[test]
fn u5_permutations() {
    let targets = [0, 2, 3, 1, 6, 4, 5, 7];
    let signs = [1., 1., 1., 1., 1., -1., -1., 1.];
    for i in 0..8 {
        let x = blade(i);
        let y = x.permute(1);
        assert_eq!(y, blade(targets[i]).scale(signs[i]));
        assert_eq!(x.permute_generators([1, 2, 0]).unwrap(), y);
        assert_eq!(y.permute(-1), x);
        assert_eq!(x.permute(3), x);
        assert_eq!(x.permute(i32::MIN), y);
        for j in 0..8 {
            assert_eq!((x * blade(j)).permute(1), y * blade(j).permute(1));
        }
    }
    for bad in [[1, 1, 0], [0, 1, 3]] {
        assert!(blade(1).permute_generators(bad).is_err());
    }
    assert_eq!(
        *ProductCliffordAlgebra::new([blade(4), blade(5)])
            .permute(1)
            .components(),
        [-blade(4), blade(6)]
    );
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
