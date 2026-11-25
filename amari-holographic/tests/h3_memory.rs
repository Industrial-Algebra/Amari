//! Exact additive trace regressions; all checked mutations must succeed.
use amari_holographic::{AlgebraConfig, BindingAlgebra, Cl3, FHRRAlgebra, HolographicMemory};

fn blades() -> [Cl3; 3] {
    [0, 1, 2].map(Cl3::unit_vector)
}
fn sequential(beta: f64) {
    let [a, b, c] = blades();
    let mut memory = HolographicMemory::new(AlgebraConfig {
        bundle_beta: beta,
        ..Default::default()
    });
    for value in [a, b, c] {
        memory.store(&Cl3::one(), &value).unwrap();
    }
    assert_eq!(*memory.trace(), a + b + c);
}
#[test]
fn u1_store_sequential_trace_is_additive_at_legacy_soft_beta() {
    sequential(0.)
}
#[test]
fn u1_store_sequential_trace_is_additive_at_hard_beta() {
    sequential(f64::INFINITY)
}
#[test]
fn u1_batch_and_merge_partitions_match_sequential_trace() {
    let values = blades();
    let pairs = values.map(|v| (Cl3::one(), v));
    for split in 0..=3 {
        let mut prefix = HolographicMemory::new(Default::default());
        let mut suffix = HolographicMemory::new(Default::default());
        prefix.store_batch(&pairs[..split]).unwrap();
        suffix.store_batch(&pairs[split..]).unwrap();
        prefix.merge(&suffix).unwrap();
        assert_eq!(prefix.item_count(), 3);
        assert_eq!(*prefix.trace(), values[0] + values[1] + values[2]);
    }
}
#[test]
fn u1_repeated_pair_doubles_its_contribution() {
    let mut m = HolographicMemory::new(Default::default());
    let v = blades()[1];
    for _ in 0..2 {
        m.store(&Cl3::one(), &v).unwrap();
    }
    assert_eq!(*m.trace(), v + v);
}
#[test]
fn u1_empty_batch_and_merge_are_noops() {
    let mut m = HolographicMemory::new(Default::default());
    let v = blades()[0];
    m.store(&Cl3::one(), &v).unwrap();
    m.store_batch(&[]).unwrap();
    m.merge(&HolographicMemory::new(Default::default()))
        .unwrap();
    assert_eq!(*m.trace(), v);
    assert_eq!(m.item_count(), 1);
}
#[test]
fn u1_fhrr_trace_preserves_real_and_imaginary_components() {
    type F = FHRRAlgebra<2>;
    let mut m = HolographicMemory::new(Default::default());
    let k = F::new([0., 1.], [1., 0.]);
    let v = F::new([1., 2.], [3., 4.]);
    m.store(&k, &v).unwrap();
    m.store(&F::identity(), &v).unwrap();
    let trace = m.trace();
    assert_eq!((trace.real(0), trace.imag(0)), (Some(-2.), Some(4.)));
    assert_eq!((trace.real(1), trace.imag(1)), (Some(4.), Some(8.)));
}
