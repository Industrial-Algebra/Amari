# H3 implementation evidence and migrations

Cargo prefix `c`: `env -u CARGO_ENCODED_RUSTFLAGS -u CARGO_BUILD_TARGET -u RUSTC_WRAPPER -u RUSTC_WORKSPACE_WRAPPER cargo +1.100.0`.

## U1 — additive trace

Independent RED reproduction on detached `48123e8a` plus the six `u1_` tests (no production overlay): `c test -p amari-holographic --test h3_memory u1_`, 6 selected; exit 101. Verbatim excerpts:
```text
all-batch partition: coefficient 1: expected 1, got 0.21364785360223157
repeated pair doubles: coefficient 2: expected 2, got 1
test result: FAILED. 1 passed; 5 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```
GREEN on the edited unit tree at parent `48123e8a`: `c test -p amari-holographic --test h3_memory u1_` and `c test -p amari-holographic --features parallel --test h3_memory u1_`, each exit 0:
```text
test u1_empty_batch_and_merge_are_noops ... ok
test u1_fhrr_trace_preserves_real_and_imaginary_components ... ok
test u1_store_sequential_trace_is_additive_at_legacy_soft_beta ... ok
test u1_repeated_pair_doubles_its_contribution ... ok
test u1_store_sequential_trace_is_additive_at_hard_beta ... ok
test u1_batch_and_merge_partitions_match_sequential_trace ... ok
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

## U2 — checked, transactional operations

At `19a867ce`, `c test -p amari-holographic --test h3_atomic` (default features), 2 selected, exit 101:
```text
batch accepted a zero divisor
accepted non-bipolar key
test result: FAILED. 0 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```
GREEN: edited tree at same parent, `c test -p amari-holographic --lib --test h3_atomic u2_`, exit 0:
```text
test memory::holographic_memory::tests::u2_checked_count_before_commit ... ok
test algebra::cl3::tests::u2_unscaled_certificate ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 154 filtered out; finished in 0.00s
test u2_invalid_key_classes_and_imaginary_values ... ok
test u2_transactional_bad_prefix_and_mixed_tracking ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

## U3 — query evidence

Parent `a4d97096`, `c test -p amari-holographic --test h3_membership` (defaults). RED 2 intended, 0 ran, compile exit 101:
```text
error[E0432]: unresolved import `amari_holographic::memory::MembershipEvidence`
error[E0599]: no method named `membership` found for struct `HolographicMemory<A>` in the current scope
error[E0599]: no method named `nearest_key` found for struct `HolographicMemory<A>` in the current scope
```
GREEN: identical command, exit 0:
```text
test u3_full_exact_evidence_and_duplicate_indices ... ok
test u3_walsh_crosstalk_without_age_gradient ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```
