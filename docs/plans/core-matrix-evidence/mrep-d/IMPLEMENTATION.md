# M-REP-D implementation evidence (2026-10-18)

Worktree core-mrep-d, branch fix/core-mrep-d-unsafe-simd-audit.

## Dispositions executed (contract PR #315, operator-approved D1-D6)

- S1 REPAIRED x2: (a) FMA precondition — every cfg gate, the std
  dispatcher (detects BOTH avx2 and fma), the no_std arm, the in-module
  tests, the bench caller, and the audit battery use the uniform
  predicate all(target_arch x86_64, avx2, fma); (b) ARITHMETIC — the
  hand-tuned pattern tables computed WRONG products for most basis pairs
  (188/448 coefficient mismatches over the 64-pair matrix; the old
  in-module test checked only e1*e2 and passed). The kernel was rewritten
  gather-based, table-driven from the blade algebra: out[k] = Sum_i
  blade_sign(i, i^k) * lhs[i] * rhs[i^k]; tables embedded as consts with
  the generator documented; cross-checked by the battery.
- S2 REPAIRED: batch fn asserts length-multiple; partial tails rejected,
  not truncated.
- S4 REMOVED (D1): aligned_alloc module + AlignedBuffer deleted (zero
  users).
- S5 DOCUMENTED (D4): per-site SAFETY comments (DIM guard, matching
  layout, no-drop-glue closure of ScalarField).
- S6 REPAIRED (D2): MetricTensor derives Clone; ptr::read sites replaced.
- S7 trait-scope REPAIRED THROUGH THREE REVISIONS (final state, R3-R4):
  the R1 narrowing premised on 'libm in DetRotor2' was WRONG — the rotor
  angle constructors call DetF32's Taylor sin/cos/atan2 (approx.rs, with
  rem_2pi argument reduction): pure fixed arithmetic, NO libm anywhere
  in the family. The trait contract was RESTORED to the strong
  cross-platform bit-exact form with the mechanism stated, one R4 scope
  qualification (cross-platform bit-identity promised for finite inputs
  and intermediates; arithmetic-NaN sign/payload is implementation-
  defined in Rust and not promised portable), and the accuracy story
  kept honest (determinism contract, not accuracy; the sqrt's false
  '< 2^-20' bound withdrawn — sqrt(100) ~= 10.84 observed; method-level
  trig error bounds predate the audit and are uncertified by it).
- S8 DOCUMENTED: GlobalAlloc proxy delegation argument at the site.
- Public API: simd module now pub; new pub fn
  select_geometric_product_impl_name() — honest runtime inventory
  (fn-pointer identity is NOT stable across compilation units; documented
  discovery during implementation).

## Battery-execution policy addendum (2026-10-18)

FMA's exact-intermediate product legitimately changes finiteness class
in the saturation regime: scalar NaN (inf + -inf under separate
rounding) vs SIMD signed-inf (exact +-2^1025 added to inf). Class
divergence WITH saturation engaged is EXPECTED-DIVERGENT-OVERFLOW —
recorded via atomic counter, surfaced in the battery summary, never a
silent pass.

## Gates (all green)

- +avx2,+fma EXECUTION row: simd_audit 9/9 (64 basis pairs, adversarial
  sets incl. subnormal-spread + +-MAX mixes, 10^4 seeded randomized
  dense, expected-divergent, runtime inventory, batch contract x2,
  oracle self-check 500 dense cases); clippy -D warnings; Miri PASSED
  on oracle_selfcheck (1756s) and tier1_all_basis_pairs (37.7s) —
  gather/FMA intrinsics ARE Miri-supported.
- +avx2 EXCLUSION row: simd_audit 2/2 (scalar-fallback inventory +
  dispatch-agrees-scalar); clippy clean; kernel compiled out.
- -avx2 production row: full amari-core 513/0; clippy; MSRV 1.89 clean.
- amari-calculus 67/0; amari (root) 9/0; amari-rewrite (neural,network)
  385/0; fmt; rustdoc; catalog regenerated (11,180 items, hash
  8421bb1c...) and drift gate green.
