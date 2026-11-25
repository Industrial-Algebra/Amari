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
