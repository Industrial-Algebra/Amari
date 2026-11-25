use amari_holographic::optical::*;
use std::f64::consts::TAU;
use OpticalRotorField as F;
fn coef(h: &BinaryHologram, k: usize) -> (f64, f64) {
    let n = h.len() as f64;
    let (mut r, mut i) = (0., 0.);
    for (x, b) in h.to_bools().into_iter().enumerate() {
        if b {
            let p = -TAU * x as f64 * k as f64 / n;
            r += p.cos() / n;
            i += p.sin() / n;
        }
    }
    (r.hypot(i), i.atan2(r))
}
fn f(p: f32, a: f32) -> F {
    F::uniform(p, a, (1, 1))
}
fn p(f: F) -> f32 {
    f.phase_at(0, 0)
}
fn fnv(seed: [u8; 8]) -> u64 {
    seed.into_iter()
        .chain([b'X', 255])
        .fold(0xcbf29ce484222325, |h, b| {
            (h ^ b as u64).wrapping_mul(0x100000001b3)
        })
}
fn main() {
    let n = 1024;
    let enc = GeometricLeeEncoder::with_frequency((n, 1), 1. / 128.);
    for a in [0., 0.25, 0.5, 0.75, 1., 2.] {
        let v = F::uniform(0.37, a, (n, 1));
        let h = enc.encode(&v);
        let (c, ph) = coef(&h, 8);
        println!(
            "LEE A={a} DC={} |C1|={c:.8} arg={ph:.5} power={:.8} api={:.8}",
            h.fill_factor(),
            c * c,
            enc.theoretical_efficiency(&v)
        );
    }
    for k in [8, 512, 0] {
        let e = GeometricLeeEncoder::with_frequency((n, 1), k as f32 / n as f32);
        let h = |ph| e.encode(&F::uniform(ph, 0.5, (n, 1)));
        println!(
            "BAND k={k} +/-phase patterns distance={} bins=({}, {})",
            h(0.37).hamming_distance(&h(-0.37)),
            k,
            (n - k) % n
        );
    }
    let alg = OpticalFieldAlgebra::new((1, 1));
    let key = f(0.2, 0.5);
    let val = f(0.4, 0.8);
    println!(
        "UNBIND amp={} expected=.8",
        alg.unbind(&key, &alg.bind(&key, &val)).amplitudes()[0]
    );
    let q = f(std::f32::consts::FRAC_PI_2, 1.);
    println!(
        "SIM inverse={} tiny={} large={}",
        alg.similarity(&q, &alg.inverse(&q)),
        alg.similarity(&f(0., 1e-6), &f(0., 1e-6)),
        alg.similarity(&f(0., 1e10), &f(0., 1e10))
    );
    println!(
        "NORM tiny={} big={} bundle_tiny={}",
        f(0., 1e-6).normalized().total_energy(),
        f(0., 1e20).normalized().total_energy(),
        alg.bundle_uniform(&[f(0., 1e-11)]).amplitudes()[0]
    );
    let mut v = f(0., 1.);
    v.scalars_mut()[0] = 2.;
    println!(
        "RAW selfsim={} variance={}",
        alg.similarity(&v, &v),
        alg.phase_variance(&v)
    );
    let t = TropicalOpticalAlgebra::new((1, 1));
    let (a, b, c) = (f(1., 1.), f(2., 1.), f(-2., 1.));
    println!(
        "LAWS tie_ab={} tie_ba={} lhs={} rhs={}",
        p(t.tropical_add(&a, &f(-1., 1.))),
        p(t.tropical_add(&f(-1., 1.), &a)),
        p(t.tropical_mul(&c, &t.tropical_add(&a, &b))),
        p(t.tropical_add(&t.tropical_mul(&c, &a), &t.tropical_mul(&c, &b)))
    );
    println!(
        "SOFT {:?}",
        t.soft_tropical_add(&a, &b, 200.).rotor_at(0, 0)
    );
    println!(
        "ATTR exact 2 -> {}",
        p(t.attractor_step(&b, &[a.clone(), b.clone()]).unwrap())
    );
    let grid = F::new(vec![0., 1., 2., 3.], vec![0., 1., 2., 3.], (2, 2));
    println!(
        "INDEX invalid (2,0) {:?} == (0,1) {:?}",
        grid.rotor_at(2, 0),
        grid.rotor_at(0, 1)
    );
    let empty = F::identity((0, 1));
    println!(
        "SHAPE min={:?} empty_dist={} reverse_ham_panics={}",
        t.tropical_add(&grid, &grid).dimensions(),
        t.normalized_phase_distance(&empty, &grid),
        std::panic::catch_unwind(|| BinaryHologram::ones((2, 2))
            .normalized_hamming_distance(&BinaryHologram::zeros((0, 1))))
        .is_err()
    );
    let mut j = serde_json::to_value(&grid).unwrap();
    j["amplitude"] = serde_json::json!([]);
    let v: F = serde_json::from_value(j).unwrap();
    let h: BinaryHologram = serde_json::from_str(r#"{"dimensions":[1,1],"data":[255]}"#).unwrap();
    println!(
        "SERDE accepted: field len={} amp_len={} hologram fill={} hamming={}",
        v.len(),
        v.amplitudes().len(),
        h.fill_factor(),
        h.hamming_distance(&BinaryHologram::zeros((1, 1)))
    );
    let seed = 0x0102030405060708u64;
    let mut cb = OpticalCodebook::new(CodebookConfig::new((2, 2), seed));
    let id = SymbolId::new("X");
    cb.register(id.clone());
    println!(
        "SEED native={:x} LE={:x} BE={:x}",
        cb.get_seed(&id).unwrap(),
        fnv(seed.to_le_bytes()),
        fnv(seed.to_be_bytes())
    );
    let restored: OpticalCodebook =
        serde_json::from_str(&serde_json::to_string(&cb).unwrap()).unwrap();
    println!(
        "CHECKPOINT same={}",
        cb.get(&id) == restored.generate(&id).as_ref()
    );
    let mut repeated = f(0.7, 1.);
    for _ in 0..28 {
        repeated = alg.bind(&repeated, &repeated);
    }
    println!(
        "DRIFT norm={} sim={}",
        repeated.scalars()[0].hypot(repeated.bivectors()[0]),
        alg.similarity(&repeated, &repeated)
    );
    for q in [3i32, -8, 504] {
        let phases = (0..n)
            .map(|x| (0.37 + TAU * (q as f64) * (x as f64) / n as f64) as f32)
            .collect();
        let h = enc.encode(&F::new(phases, vec![0.5; n], (n, 1)));
        let k = (8 + q) as usize;
        println!(
            "RAMP q={q} k={k} fill={} |Ck|={:.8}",
            h.fill_factor(),
            coef(&h, k).0
        );
    }
    if !cfg!(debug_assertions) {
        let d = (usize::MAX / 2 + 1, 2);
        println!(
            "PRODUCT field len={} holo len={} dims={d:?}",
            F::identity(d).len(),
            BinaryHologram::zeros(d).len()
        );
    }
}
