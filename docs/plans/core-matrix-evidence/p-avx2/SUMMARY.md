# P-AVX2 Execution Gate — EXECUTED GREEN (2026-10-23)

Matrix §5 row (line 253): host + explicit `+avx2,+fma`, representative
configs — compile, runtime SIMD inventory, independent arithmetic
checks vs scalar expectations. Per the 2026-10-18 addendum, the M-REP-D
battery (`tests/simd_audit.rs`, rewritten table-driven kernel + exact
bigint oracle) IS the required evidence.

- develop `9986e89`, stable pinned `88d9e12ae…`, `--locked`.
- **5/5 rows exit 0**: battery ×{default, minimal, native, all-features}
  (10/10 tests each: basis-pair matrix vs oracle, adversarial sets,
  EXPECTED-DIVERGENT class, runtime inventory) + full default suite
  (522 tests green — counted from the committed default-full log; the 515 in the first draft was stale, carried from pre-#341 develop).
- Runtime inventory: `runtime_inventory` asserts dispatcher selects
  `"avx2+fma"` and dispatch ≡ direct kernel on all 8 coefficients.
- Runner: `avx2-gate.sh` — TOTAL override discipline (evidence AND
  build artifacts route under OUT_ROOT_OVERRIDE), toolchain-pin
  fail-closed, header-excluded failure count.

**D-A disposition for AVX2: RETAIN (retain-and-execute satisfied).**
The advertised AVX2 promise is proven: not withdrawn, not narrowed.

## Self-noted defect in the runner itself (fixed before commit)

First summary printed `failed=1` while every row exited 0 — the
failure counter counted the results TSV *header* (`exit` != `"0"`).
Fixed to skip the header; the all-green rerun above is the committed
evidence. Standing lesson family: count from authoritative rows only.
