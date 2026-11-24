use amari_holographic::memory::{Resonator as R, ResonatorConfig as RC};
use amari_holographic::{AlgebraConfig, BindingAlgebra, Cl3, HolographicMemory as M};
fn main() {
    let (a, b, c) = (
        Cl3::vector(1., 0., 0.),
        Cl3::vector(0., 1., 0.),
        Cl3::vector(0., 0., 1.),
    );
    let one = Cl3::one();
    let z = Cl3::zero();
    let mut m = M::new(AlgebraConfig::default());
    let r = m.retrieve(&a);
    println!(
        "empty: member={} confidence={} norm={} valid={}",
        m.probably_contains(&a),
        r.confidence,
        r.value.norm(),
        r.verify_result_validity()
    );
    m.store(&a, &b);
    let bad = m.retrieve(&z);
    println!(
        "partial: all-false == all-true: {}",
        m.query_partial(&a, &[false; 8]).raw_value == m.query_partial(&a, &[true; 8]).raw_value
    );
    println!(
        "single: recall={} absent_member={} singular_norm={} singular_conf={:.6} valid={}",
        m.retrieve(&a).value.similarity(&b),
        m.probably_contains(&c),
        bad.value.norm(),
        bad.confidence,
        bad.verify_result_validity()
    );
    let k = one.scale(2.).add(&a);
    let mut mixed = M::new(Default::default());
    mixed.store(&k, &b);
    println!(
        "mixed: raw={:?} expected={:?}",
        mixed.retrieve(&k).raw_value.to_coefficients(),
        b.to_coefficients()
    );
    let left = R::new(vec![a, a.scale(-1.)], Default::default()).unwrap();
    let right = R::new(vec![b], Default::default()).unwrap();
    let f = left.factorize(&a.bind(&b), &right);
    println!(
        "factor: sim_to_a={} recon={} converged={} iterations={}",
        f.factor_a.similarity(&a),
        f.reconstruction_similarity,
        f.converged,
        f.iterations
    );
    let clean = right.cleanup(&a);
    println!(
        "orthogonal: sim={} converged={} iterations={}",
        clean.final_similarity, clean.converged, clean.iterations
    );
    let inf = R::new(
        vec![a, b],
        RC {
            initial_beta: f64::INFINITY,
            final_beta: f64::INFINITY,
            max_iterations: 1,
            ..Default::default()
        },
    )
    .unwrap()
    .cleanup(&a);
    println!(
        "infinite-beta exact match: index={} expected=0",
        inf.best_match_index
    );
    let cfg = AlgebraConfig {
        bundle_beta: 0.,
        ..Default::default()
    };
    let mut s = M::new(cfg.clone());
    for v in [a, b, c] {
        s.store(&one, &v);
    }
    let mut s2 = M::new(cfg.clone());
    s2.store_batch(&[(one, b), (one, c)]);
    let mut s1 = M::new(cfg);
    s1.store(&one, &a);
    s1.merge(&s2);
    println!(
        "soft: sequential={:?} merged={:?} additive={:?}",
        s.trace().to_coefficients(),
        s1.trace().to_coefficients(),
        a.superpose(&b)
            .unwrap()
            .superpose(&c)
            .unwrap()
            .to_coefficients()
    );
    let mut t = M::with_key_tracking(Default::default());
    t.merge(&m);
    println!(
        "mixed tracking: count={} consistent={}",
        t.item_count(),
        t.verify_consistency()
    );
    let mut h = M::new(AlgebraConfig {
        bundle_beta: f64::INFINITY,
        ..Default::default()
    });
    h.store(&one, &a);
    h.store(&c, &one);
    let i = h.capacity_info();
    println!(
        "hard: recall2={} capacity={} snr={} near={} count={}",
        h.retrieve(&c).value.similarity(&one),
        i.theoretical_capacity,
        i.estimated_snr,
        i.near_capacity,
        i.item_count
    );
    type P = amari_holographic::ProductCliffordAlgebra<32>;
    let key = |j: u32| {
        P::from_vec(
            (0u32..32)
                .map(|i| {
                    if (i & j).count_ones() % 2 == 0 {
                        a
                    } else {
                        a.scale(-1.)
                    }
                })
                .collect(),
        )
        .unwrap()
    };
    let mut pm = M::<P>::new(Default::default());
    for j in 0..8 {
        pm.store(&key(j), &P::identity());
    }
    let r = pm.retrieve(&key(0));
    let i = pm.capacity_info();
    println!("default ProductCl3x32: count={} cap={} near={} snr={:.6} confidence={:.6} oldest_sim={:.6} newest_sim={:.6} absent={} ",i.item_count,i.theoretical_capacity,i.near_capacity,i.estimated_snr,r.confidence,r.value.similarity(&P::identity()),pm.retrieve(&key(7)).value.similarity(&P::identity()),pm.probably_contains(&key(8)));
}
