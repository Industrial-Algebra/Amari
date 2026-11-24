//! Audit observation, not an implementation or a future conformance test.
use amari_core::Multivector;
use amari_holographic::algebra::{
    BindingAlgebra as B, Cl3, CliffordAlgebra as C, GeometricAlgebra as G,
    ProductCliffordAlgebra as P,
};
use std::panic::catch_unwind;

type M = Multivector<3, 0, 0>;
const MASK: [usize; 8] = [0, 1, 2, 4, 3, 5, 6, 7];
fn blade(i: usize) -> Cl3 {
    let mut x = Cl3::new_zero();
    x.set_coeff(i, 1.);
    x
}
fn lift(x: Cl3) -> M {
    let mut y = M::zero();
    for (i, m) in MASK.into_iter().enumerate() {
        y.set(m, x.coeff(i));
    }
    y
}
fn main() {
    let b = (0..8).map(blade).collect::<Vec<_>>();
    let (
        mut gp,
        mut assoc,
        mut ip,
        mut op,
        mut rev,
        mut conj,
        mut dual,
        mut cyc,
        mut lifted,
        mut par,
    ) = (0, 0, 0, 0, 0, 0, 0, 0, 0, 0);
    let mut pairs = Vec::new();
    for (i, &x) in b.iter().enumerate() {
        let a = lift(x);
        let mut c = a.reverse();
        if MASK[i].count_ones() % 2 == 1 {
            c = c * (-1.);
        }
        rev += usize::from(lift(x.reverse()) != a.reverse());
        conj += usize::from(lift(x.conjugate()) != c);
        dual += usize::from(lift(x.dual()) != a.hodge_dual().unwrap());
        for (j, &y) in b.iter().enumerate() {
            let z = lift(y);
            let xy = x * y;
            let refxy = a.geometric_product(&z);
            // Independent signed-word reduction: count inversions crossing the boundary, cancel equal Euclidean letters.
            let swaps = (0..3)
                .filter(|k| MASK[i] & (1 << k) != 0)
                .map(|k| (MASK[j] & ((1 << k) - 1)).count_ones())
                .sum::<u32>();
            let mut expected = M::zero();
            expected.set(MASK[i] ^ MASK[j], if swaps % 2 == 0 { 1. } else { -1. });
            assert_eq!(expected, refxy);
            if lift(xy) != refxy {
                gp += 1;
                pairs.push((i, j));
            }
            let p = P::new([x, Cl3::one()]);
            let q = P::new([y, Cl3::one()]);
            lifted += usize::from(lift(*p.bind(&q).component(0).unwrap()) != refxy);
            par += usize::from(lift(*p.par_component_product(&q).component(0).unwrap()) != refxy);
            ip += usize::from(lift(x.inner_product(&y)) != a.inner_product(&z));
            op += usize::from(lift(x.outer_product(&y)) != a.outer_product(&z));
            cyc += usize::from(xy.permute(1) != x.permute(1) * y.permute(1));
            for &w in &b {
                assoc += usize::from((x * y) * w != x * (y * w));
            }
        }
    }
    println!("mismatches: gp={gp}/64 product={lifted}/64 rayon_product={par}/64 associativity={assoc}/512 inner={ip}/64 outer={op}/64 reverse={rev}/8 conjugate={conj}/8 Hodge={dual}/8 cyclic_hom={cyc}/64");
    println!("wrong GP pairs (local indices): {pairs:?}");
    println!(
        "(e1 e3)e2 vs e1(e3 e2): {:?} vs {:?}",
        (b[1] * b[3] * b[2]).coefficients(),
        (b[1] * (b[3] * b[2])).coefficients()
    );
    for t in [1., 2.] {
        let x = Cl3::scalar(t) + b[1];
        let inv = x.inverse().unwrap();
        println!(
            "({t}+e1)*reported_inverse = {:?}; core inverse {:?}",
            (x * inv).coefficients(),
            lift(x).inverse().map(|m| m.to_vec())
        );
    }
    println!(
        "1 wedge e1={:?}, e1 inner I={:?}, perm(e12)={:?}",
        b[0].outer_product(&b[1]).coefficients(),
        b[1].inner_product(&b[7]).coefficients(),
        b[4].permute(1).coefficients()
    );
    let f = C::<3, 0, 0>::new(lift(b[4]));
    println!(
        "dual(e12) local/full/core e3 coeff: {}, {}, {}",
        b[4].dual().coeff(3),
        f.dual().get(4).unwrap(),
        f.inner().hodge_dual().unwrap().get(4)
    );
    let null = C::<0, 0, 1>::unit_vector(0).unwrap();
    let neg = C::<0, 1, 0>::unit_vector(0).unwrap();
    println!("self-sim negative={}, null={}; null dual={:?}, core is_err={}; pure-null random_versor(1) inverse is_err={}",neg.similarity(&neg),null.similarity(&null),null.dual().to_coefficients(),null.inner().hodge_dual().is_err(),C::<0,0,1>::random_versor(1).inverse().is_err());
    for x in [1e-200, 1e200] {
        let v = Cl3::scalar(x);
        let f = C::<3, 0, 0>::scalar(x);
        println!("x={x:e}: norm={} normalized scalar={:?} similarity(self)={} (full: norm={:e}, self-sim={})",v.norm(),v.normalize().ok().map(|v|v.coeff(0)),v.similarity(&v),f.norm(),f.similarity(&f));
    }
    let repeated = [P::<1>::identity(), P::<1>::identity()];
    println!(
        "[1,1] beta=0 sequential={}, rayon={}",
        P::bundle_all(&repeated, 0.).unwrap().get(0).unwrap(),
        P::par_bundle_all(&repeated, 0.).unwrap().get(0).unwrap()
    );
    let p = P::new([Cl3::scalar(2.), Cl3::scalar(3.)]);
    println!(
        "shuffle([0,2]) scalars = {:?}",
        p.shuffle_components(&[0, 2])
            .components()
            .map(|c| c.coeff(0))
    );
    println!(
        "K=0 permute(0) panics: {}",
        catch_unwind(|| P::<0>::identity().permute(0)).is_err()
    );
}
