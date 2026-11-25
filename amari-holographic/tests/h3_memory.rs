// SPDX-License-Identifier: Apache-2.0 OR MIT

//! H3 memory contract regressions: additive traces, fallibility and membership evidence.

use amari_holographic::{AlgebraConfig, BindingAlgebra, Cl3, HolographicMemory};

const TOL: f64 = 1.0e-12;

fn assert_coefficients_close(actual: &[f64], expected: &[f64], context: &str) {
    assert_eq!(
        actual.len(),
        expected.len(),
        "{context}: coefficient length mismatch"
    );
    for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
        assert!(
            (actual - expected).abs() <= TOL,
            "{context}: coefficient {index}: expected {expected}, got {actual}"
        );
    }
}

fn e1() -> Cl3 {
    Cl3::unit_vector(0)
}

fn e2() -> Cl3 {
    Cl3::unit_vector(1)
}

fn e3() -> Cl3 {
    Cl3::unit_vector(2)
}

fn cl3_memory(config: AlgebraConfig) -> HolographicMemory<Cl3> {
    HolographicMemory::new(config)
}

fn expected_trace(values: &[Cl3]) -> Cl3 {
    let mut trace = Cl3::zero();
    for value in values {
        trace = trace.superpose(value).expect("test trace superpose");
    }
    trace
}

#[test]
fn u1_store_sequential_trace_is_additive_at_legacy_soft_beta() {
    let mut memory = cl3_memory(AlgebraConfig {
        bundle_beta: 0.0,
        ..AlgebraConfig::default()
    });
    let identity = Cl3::identity();
    memory.store(&identity, &e1());
    memory.store(&identity, &e2());
    memory.store(&identity, &e3());

    assert_coefficients_close(
        &memory.trace().to_coefficients(),
        &expected_trace(&[e1(), e2(), e3()]).to_coefficients(),
        "sequential additive trace at beta=0",
    );
}

#[test]
fn u1_store_sequential_trace_is_additive_at_hard_beta() {
    let mut memory = cl3_memory(AlgebraConfig {
        bundle_beta: f64::INFINITY,
        ..AlgebraConfig::default()
    });
    let identity = Cl3::identity();
    memory.store(&identity, &e1());
    memory.store(&identity, &e2());
    memory.store(&identity, &e3());

    assert_coefficients_close(
        &memory.trace().to_coefficients(),
        &expected_trace(&[e1(), e2(), e3()]).to_coefficients(),
        "sequential additive trace at beta=infinity",
    );
}

#[test]
fn u1_batch_and_merge_partitions_match_sequential_trace() {
    let identity = Cl3::identity();
    let values = [e1(), e2(), e3()];
    let expected = expected_trace(&values).to_coefficients();

    let mut all_batch = cl3_memory(AlgebraConfig::default());
    all_batch.store_batch(&[
        (identity, values[0]),
        (identity, values[1]),
        (identity, values[2]),
    ]);
    assert_coefficients_close(
        &all_batch.trace().to_coefficients(),
        &expected,
        "all-batch partition",
    );

    let mut prefix_batch = cl3_memory(AlgebraConfig::default());
    prefix_batch.store_batch(&[(identity, values[0]), (identity, values[1])]);
    prefix_batch.store(&identity, &values[2]);
    assert_coefficients_close(
        &prefix_batch.trace().to_coefficients(),
        &expected,
        "batch-prefix partition",
    );

    let mut suffix_batch = cl3_memory(AlgebraConfig::default());
    suffix_batch.store(&identity, &values[0]);
    suffix_batch.store_batch(&[(identity, values[1]), (identity, values[2])]);
    assert_coefficients_close(
        &suffix_batch.trace().to_coefficients(),
        &expected,
        "batch-suffix partition",
    );

    let mut merged = cl3_memory(AlgebraConfig::default());
    merged.store(&identity, &values[0]);
    let mut tail = cl3_memory(AlgebraConfig::default());
    tail.store_batch(&[(identity, values[1]), (identity, values[2])]);
    merged.merge(&tail);
    assert_coefficients_close(
        &merged.trace().to_coefficients(),
        &expected,
        "merge partition",
    );
    assert_eq!(merged.item_count(), 3);
}

#[test]
fn u1_repeated_pair_doubles_its_contribution() {
    let mut memory = cl3_memory(AlgebraConfig::default());
    let key = Cl3::identity();
    let value = e2();
    let bound = key.bind(&value);

    memory.store(&key, &value);
    memory.store(&key, &value);

    let doubled = bound
        .superpose(&bound)
        .expect("test doubled binding superpose");
    assert_coefficients_close(
        &memory.trace().to_coefficients(),
        &doubled.to_coefficients(),
        "repeated pair doubles",
    );
}

#[test]
fn u1_empty_batch_and_merge_are_noops() {
    let mut memory = cl3_memory(AlgebraConfig::default());
    let key = Cl3::identity();
    memory.store(&key, &e1());
    let before_trace = memory.trace().to_coefficients();
    let before_count = memory.item_count();

    memory.store_batch(&[]);
    let empty = cl3_memory(AlgebraConfig::default());
    memory.merge(&empty);

    assert_eq!(memory.item_count(), before_count);
    assert_coefficients_close(
        &memory.trace().to_coefficients(),
        &before_trace,
        "empty batch/merge no-op",
    );
}

#[test]
fn u1_fhrr_trace_preserves_real_and_imaginary_components() {
    use amari_holographic::FHRRAlgebra;

    type Fhrr = FHRRAlgebra<3>;
    let mut memory = HolographicMemory::<Fhrr>::new(AlgebraConfig::default());
    let key_a = Fhrr::random_unitary();
    let key_b = Fhrr::random_unitary();
    let value_a = Fhrr::new([0.5, -1.0, 2.0], [0.25, 1.5, -0.75]);
    let value_b = Fhrr::new([-0.25, 0.75, -1.5], [2.0, -0.5, 0.125]);

    memory.store(&key_a, &value_a);
    memory.store(&key_b, &value_b);

    let expected = key_a
        .bind(&value_a)
        .superpose(&key_b.bind(&value_b))
        .expect("test FHRR expected trace");
    let trace = memory.trace();
    for index in 0..3 {
        assert!(
            (trace.real(index).unwrap() - expected.real(index).unwrap()).abs() <= TOL,
            "FHRR real component {index}: expected {}, got {}",
            expected.real(index).unwrap(),
            trace.real(index).unwrap()
        );
        assert!(
            (trace.imag(index).unwrap() - expected.imag(index).unwrap()).abs() <= TOL,
            "FHRR imaginary component {index}: expected {}, got {}",
            expected.imag(index).unwrap(),
            trace.imag(index).unwrap()
        );
    }
}
