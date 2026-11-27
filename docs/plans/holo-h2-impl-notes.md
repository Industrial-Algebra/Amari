# H2 implementation and migration evidence

`c` means `env -u CARGO_ENCODED_RUSTFLAGS -u CARGO_BUILD_TARGET -u RUSTC_WRAPPER -u RUSTC_WORKSPACE_WRAPPER cargo +1.100.0` from the H2 worktree root.

## U1 — binding keys and general division

At parent `c0e66483` plus the initial three tests, **before** implementation: `c test -p amari-holographic --test h2_keys` (defaults), exit 101. Verbatim runtime RED excerpts:

```text
assertion failed: F::new([0., 3.], [0., 4.]).normalize_unitary().is_err()
  left: [12.0, -36.0]
 right: [3.0, -4.0]
  left: [0.0, 8.0, 9.0, 0.0]
 right: [0.0, 2.0, 1.0, 0.0]
test result: FAILED. 0 passed; 3 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Same command after implementation, same parent plus U1 changes: 7 passed, exit 0 (`general_key_does_not_rebind`, `nonunit_phases_are_not_inverted_by_correlation`, `projection_rejects_a_zero_bin`, `dense_complex_convolution`, `typed_admission_is_not_general_invertibility`, `singular_and_mixed_clifford_keys`, `sampler_reports_domain`). `c test -p amari-holographic --all-features`: 195 passed, 0 failed, 17 ignored, exit 0.

**Migration:** `TypedKey::try_new(element)?` admits a retrieval key; call `bind_key` / `unbind_key` on the validated key. For general algebraic division call `unbind_general(&key,&bound)?`. Use `correlation` when the original adjoint/rebinding was intended instead. A reciprocal is not admission to H3 membership. Full-wrapper `CliffordAlgebra::random_versor` is now fallible, including for positive factor counts in purely null signatures. Rebuild derived values from original operands if previous code stored the result of nonunit rebinding as a recovered value; there is no blanket reinterpretation of old results.
