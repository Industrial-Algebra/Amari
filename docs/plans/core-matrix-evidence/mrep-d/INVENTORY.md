# M-REP-D measured inventory (raw evidence)

Develop dedd1f6 + #314. Commands and full site list per contract §1.
Reproduced 2026-10-17:
- unsafe site census (CORRECTED R1 — root src/ and tests/ now included):
  7 genuine sites/files — core simd.rs, aligned_alloc.rs; calculus
  metric.rs, connection.rs; ROOT amari crate src/deterministic/
  (unsafe trait Deterministic + impls); amari-rewrite/tests/language_metered.rs
  (GlobalAlloc proxy). Prior census undercounted by missing root crate + tests.
- false positives dispositioned: discovery metadata fields (8),
  parser test fixture string (2), gpu subset_len field name (5)
- dead infrastructure: aligned_alloc module (0 users), AlignedBuffer (0 users)
- construction invariant verified: from_coefficients asserts len==BASIS_COUNT
  (lib.rs:238-244), boxed slice — S1 in-bounds argument holds

R1 corrections (2026-10-18): FMA precondition discovered (fmadd under avx2-only cfg);
transmute_copy runtime-check/ownership-duplication corrected; census commands fixed
(rg -E, full target coverage). Reviewer reproduced: rustc +stable -C target-cpu=x86-64
-C target-feature=+avx2 --print cfg -> fma absent.
