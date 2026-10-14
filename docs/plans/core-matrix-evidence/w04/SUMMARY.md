# CORE-W04 — rotor domains, duality, grade-type enforcement (evidence summary)

Contract: docs/plans/2026-10-13-amari-core-w04-rotor-duality-contract.md (revision 9,
8-round independent review: R1 5P1/9P2 -> R8 0P1/0P2) + consumer-impact companion
(revision 9, 8-round review; compilation-honest per-operation disclosure).
Operator-approved 2026-10-13 after explicit hold-and-discuss + external-consumer
framing (33k downloads, ^0.24.1 pins).

Branch: fix/core-w04-rotor-duality from develop 50e9b46. Lock eb2442dc.
Red: 11 findings reproduced (findings.log); baseline 222 lib tests.

## Stages (all pinned-recipe, --locked, archived stage-*.log)
- S1 errors/privacy/Result: error enums; mv private + escapes + TryFrom; four
  fallback deletions; uniform R†/certified-N inverse; N-gates; consumer migration.
  223 lib / 397 total.
- S2 log/powers (+no_std repair of pre-existing W03 breakage): §2 a–d exact —
  scalar-first branches; exact B² classification; element-log scales; certified-N
  hyperbolic (exact-rational q, δ_num/N, ln1p/decomposition branches, no β/w);
  range-safe intermediates; power_integer routing + output gates. 231 lib / 420.
- S3 slerp/from_vectors/reflect: coefficient-dot sign rule + exact endpoints;
  exact 2×2-minor proportionality direction test; candidate trichotomy
  (NullCandidate/SheetChange); definite-2D-only antipodal resolution (indefinite
  2D ±e12 has negative reverse norm, `e12·e1·e12 = −e1`, not the identity
  action — R1-F6; Err(Antipodal)); grade-
  involution hyperplane reflect + reflect_line. 234 lib / 444.
- S4 duals/components: formal_complement (all signatures); metric hodge_dual
  with DegeneratePseudoscalar; generic typed dual deleted (compile_fail doctest
  pin); concrete 3D typed duals; lexicographic component map (relativistic
  boost-bivector fix fallout); e_ij strict. 234 lib / 466.

## Final matrix (final-*.log)
- amari-core: 466 tests / 17 suites on stable AND nightly, identical
  inventories; clippy 0 both; workspace all-targets 0; no-default 0; fmt clean.
  Test lineage: 385 (W03) -> 466.
- Catalog dfd35092…, 11,134 items — only amari-core.json + manifest.json
  changed (CAT1 isolation holding).
- Workspace clippy verified green directly (the S4 report's tropical flag was
  a stale-context artifact).
- Evidence paths scrubbed from creation ($AMARI/$HOME placeholders).

## Disclosed deviations (all in stage summaries)
S2: ln_ratio combined form (witness rapidity correctly rounded); binary gate via
exact scalar-part sign (multi-plane rounding) [SUPERSEDED by R1-F5: the checked
binary-power boundary now requires the full positive-scalar exact `R·R†`
certificate, so rounded multi-plane composites are `Err(Degenerate)`];
from_bivector non-simple ->
Degenerate (series N non-scalar under rounding); mixed-component fixture in
unit test (1-ulp conditioning); m+1=6 ulp budget; odd grades -> Uncertified
superset; integral-power fallback only on Uncertified; N-certified-before-
normalize ordering. S3: indefinite-2D antipodal excluded (e12 negative reverse
norm, `e12·e1·e12 = −e1`; not the identity action); uncertifiable square ->
NotNormalizable; C=0 path retained (proven implied).
S4: Bivector get/Index generic via general map ([3,5,6] literal removed);
from_components generic-delegating (4D third arg e14, documented; relativistic
boost map corrected); compile_fail doctest + grep (no trybuild); enumerative
.expect (all non-degenerate); audit-repro stand-alone crate untouched.

## R1 remediation (2026-10-14)

Review round 1 on `6b3c933` produced 7 P1 findings; all were remediated on the
same branch (see `r1-remediation-*.log`; recipe pinned with
`RUSTC=$(rustup which --toolchain stable rustc)` and the matching stable
`RUSTDOC`):

- **F1** exact helpers now guard `BASIS_COUNT > 64` and return the checked
error (`Degenerate`/`Uncertified`) instead of slicing past the 64-slot table
(7D `inverse`/`apply`/`power` no longer panic; scalar-only 7D `logarithm` keeps
the dimension-independent `w = 1 -> 0` route).
- **F2** `inverse`/`apply` divide each coefficient by the exact `N` dyadic
with one rounding, never by a pre-rounded `f64`.
- **F3** `ExactSigned::ratio_to_f64` is exact rational division with a sticky
remainder bit (correctly rounded `delta`/rapidity).
- **F4** exp-route powers certify the un-normalized exponential output and
preserve the element scale [F8 extends the same certify-without-normalization
rule to the `slerp` interior].
- **F5** checked binary-power boundaries use the full positive-**scalar**
reverse-norm certificate (non-scalar exact `N` -> `Degenerate`).
- **F6** antipodal routing is decided by signature before normalization:
definite 2D only; Cl(1,1,0) -> `Err(Antipodal)` (derivation corrected in
`stage-s3-summary.md`).
- **F7** `from_axis_angle` normalizes from the exact scalar square, so
`1e200`/`1e-200` axes no longer overflow/underflow.

Post-remediation matrix: **239 lib / 475 total** (was 234 / 466), clippy 0,
fmt clean, rustdoc 0, no-default 0, workspace all-targets 0.

## R2 remediation (2026-10-14)

Review round 2 on `d173ab8` produced 3 P1 / 1 P2 / 2 P3 findings; all were
remediated on the same branch (see `r2-remediation-*.log`):

- **F8** the `slerp` interior `A·(A⁻¹B*)^t` certifies without normalizing (the
same scale-preserving rule as `power`), so `identity.slerp([4,0], 0.5)` returns
`[2, 0]`; literal endpoints and `A = B` are unchanged.
- **F9** checked rotor-producing outputs (`inverse`, `power`/`power_integer` exp
outputs) and the `apply` quotient reject non-finite coefficients and zero-loss
collapse: a stored scalar `2⁻¹⁰⁷⁰` (exact `N = 2⁻²¹⁴⁰`) now returns
`Err(Degenerate)` for `inverse`/`power_integer(−1)`/`apply` instead of `Ok`
holding `+inf` or zero.
- **F10** `from_axis_angle` normalizes from the symbolic exact scalar square
(`sqrt(|σ|) = γ·2^q`, `q` never materialized), so an overflowing intermediate
root no longer collapses `[MAX, MAX, 0]` to the identity; it matches `[1,1,0]`
bit-exact.
- **F11** the consumer companion discloses the rounded multi-plane power
rejection by exact certification and the compose-chain migration.
- **F12** these records are annotated: the false indefinite-`±e12` identity
line and the superseded scalar-part binary gate are corrected here, and the S2
summary's superseded claims carry `[SUPERSEDED by R1-F5: …]` pointers.
- **F13** the Cl(2,2,0) dyadic battery comment is corrected and pins its true
exact reverse norm `289/64` (both planes elliptic, each factor `N = 17/8`).

Post-remediation matrix: **239 lib / 478 total** (was 239 / 475), clippy 0,
fmt clean, rustdoc 0, no-default 0, workspace all-targets 0.

## R3 remediation (2026-10-14)

Review round 3 on `c495ab5` produced 2 P1 / 1 P2 findings; all were remediated
on the same branch (see `r3-remediation-*.log`):

- **F14** `apply` computes each image coefficient as the exact dyadic quotient
  `(R·A·R†)/N` (two exact multivector products then the sticky-bit divider,
  rounded once at the destination); the post-rounding zero-loss check is
  deleted. The former `Err(Degenerate)` for the angle-2 Cl(2,1,0) boost on
  `2^-1074(e1−e3)` is now `Ok(exact zero image)` (correctly rounded), and the
  angle-0.5 variant stays the min-subnormal image. **Pin revision (R3-F14):** a
  tiny *scalar* rotor's action is the identity map, so `tiny(2^-1070).apply(A)
  = Ok(A)` exactly — the R2 expectation (already `Err`) was the orchestrator's
  error; `inverse`/`power_integer(−1)` stay `Err(Degenerate)` (their outputs
  genuinely overflow). The non-finite-quotient gate is retained and an
  overflowing-quotient battery is pinned (angle-30 boost, `N>0`, near-max
  input).
- **F15** `from_vectors` normalizes through the symbolic exact scalar square
  (`sqrt(|σ|) = γ·2^q`), the same machinery as `from_axis_angle`; the earlier
  materialized-`β` asymmetry is closed. The Cl(0,2,0) subnormal fixture
  `e1 → 2^-1074(e1+e2)` now acts as the unit direction `[1/√2, 1/√2]`, and the
  direction normalization is bit-exactly scale-invariant over
  `{1, 2^-100, 2^-600, 2^-1074, 2^500, 2^1020}` in both `Cl(3,0,0)` and
  `Cl(0,3,0)` (the overflow-scale case succeeds symbolically). The `stage-s3`
  normalization deviation is annotated `[SUPERSEDED … F15]`.
- **F16** the consumer companion restates the multi-plane power rejection per
  error kind: `power(0) = Ok(identity)`, `power(1) = Ok(clone)`,
  `power(−1)`/`power(2) = Err(Degenerate)`, `power(0.5) = Err(Uncertified)` for
  `r12(0.3)∘r34(0.7)`; the compose-chain migration is qualified to supported
  integer-power workflows. A battery pins all five behaviors.

## R4 remediation (2026-10-14)

Review round 4 on `286a001` produced 3 P1 findings, all independently
reproduced by the orchestrator (Fraction-oracle mismatches, the matrix
conversion panic, and the NaN-dot mis-selection); all were remediated on the
same branch (see `r4-remediation-*.log`):

- **F17** `apply`'s exact sandwich now accumulates the triple product
  `R_i·A_j·R†_l` **directly** in the wide (`2*WORDS+1` = 133-word)
  accumulator and divides from the wide coefficient, instead of collapsing an
  intermediate `R·A` to the 66-word single-product dyadic form. **Root cause:**
  the F17 fixture rotor spans ~1 down to `2^-1070`, so the exact triple
  numerator spans ~6300 bits — beyond the 66-word (4224-bit) window — and the
  old fallback silently substituted floating arithmetic (wrong e1/e3 by one
  ulp plus a spurious e123). The floating fallback is deleted: a non-finite
  input coefficient now returns `Err(Degenerate)`. A mixed-magnitude sweep is
  pinned bit-exactly against a Python-`Fraction` oracle (provenance in the
  test comment); the fixture pins `e1 9.55336489125606e307`, `e3
  1.300000000000002`, `e123 0.0`.
- **F18** `to_rotation_matrix -> Result<[[f64; 3]; 3], RotorError>`
  propagates the certified-`N` action gate; the unconditional `.expect` on the
  3D basis action is gone. The compose-to-zero fixture (`4^-535`
  self-composed to the exact zero rotor) returns `Err(Degenerate)` instead of
  panicking. Workspace grep found only the two `amari-core/tests/rotors.rs`
  call sites (both migrated with justification-`.expect`); there are no
  relativistic/gpu/automata/network/discovery callers. The discovery catalog
  signature entry was regenerated.
- **F19** `slerp`'s representative sign is computed by exact dyadic
  accumulation (`exact_dot_sign`), so the overflowing `NaN` dot selects the
  correct `B* = -B`: the fixture's `slerp(1)` returns `-B` bit-exact (the
  interior path stays on the selected branch and is pinned); the normal-scale
  differing-sign battery is unchanged. Non-finite coefficients keep the
  floating comparison.
- **Trivial** `amari-core/src/property_tests.rs` dropped the unused
  `#[cfg(not(feature = "std"))] use alloc::format;`, keeping the no-default
  all-targets clippy clean without touching default-feature behavior.

Post-remediation matrix: **240 lib / 486 total** amari-core
(17 binaries), clippy 0 (default and no-default all-targets), fmt clean,
rustdoc 0, no-default 0, workspace all-targets 0, consumer suites
(relativistic/enumerative/gpu/automata) 596 passed, catalog regenerated
(`428fbd18…`).

## Status
IMPLEMENTATION COMPLETE AND STAGED. R1 returned 7 P1, R2 returned 3 P1 / 1 P2 /
2 P3, R3 returned 2 P1 / 1 P2, and R4 returned 3 P1; all were remediated in the
same worktree (2026-10-14) and the full pinned matrix re-ran green. Awaiting
re-review / operator go-no-go.
