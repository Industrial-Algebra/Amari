# M-REP-D — Closure Record

**Merged:** contract PR #315 → develop `78f307a` (2026-10-18); implementation
PR #317 → develop `e8183b1` (2026-10-18); implementation branch tip `bc3aa9b`
(develop merge + catalog regen after #315/#316 landed mid-flight).

## Headline

**The AVX2 geometric-product kernel computed WRONG products.** 188/448
coefficient mismatches over the full 64-basis-pair matrix: the hand-tuned
pattern tables followed a different coefficient-ordering convention than the
library's bitmask order, and the only pre-existing test checked `e1·e2` — one
of the few pairs that happened to work. The kernel was rewritten gather-based
and table-driven from the blade algebra (`out[k] = Σᵢ blade_sign(i, i^k)·lhs[i]·rhs[i^k]`,
SIGN_LO/SIGN_HI consts with a documented generator), cross-checked by an exact
`num-bigint` oracle carrying its own 500-case self-check.

The FMA precondition was wrong on every gate: `_mm256_fmadd_pd` requires
`{avx2, fma}` — `+avx2` alone does NOT enable `fma` (verified
`rustc --print cfg`). All cfg gates, the runtime dispatcher, no_std arm,
tests, bench caller, and battery now use the uniform
`all(target_arch = "x86_64", target_feature = "avx2", target_feature = "fma")`
predicate; the dispatcher detects BOTH features.

## Implementation loop record (9 rounds, cap 12; convergence = one round with 0 P1/P2)

| Round | Verdict | Finding → remediation |
|---|---|---|
| R1 | 1 P1 / 4 P2 | `ulp(MAX)` was INFINITY (next pattern is inf) making saturated bounds vacuous; `M==MAX` false overflow signal; oracle returned 0 for `MIN_POSITIVE×0.5` (`2f64.powi(-1075)` underflows; chunked `ldexp_f64`, pinned `1.1125369292536007e-308`); ordering-broken summary test; attestations cited nonexistent libm. Commit `6c4932e`. |
| R2 | 2 P2 | sqrt's `< 2^-20` claim FALSE — `sqrt(100)→10.840435028076172` (4 Newton iterations from the raw input don't converge); docs still cited libm. Commit `cec7f77`. |
| R3 | 1 P2 | **The libm premise itself was WRONG** — `DetRotor2` calls `DetF32`'s Taylor `sin`/`cos`/`atan2` (approx.rs, `rem_2pi` reduction); NO libm anywhere in the family. Trait contract RESTORED to the strong cross-platform form with the mechanism stated. Commit `c73bb83`. |
| R4 | 1 P2 / 1 P3 | Restored universal bit-exactness exceeded Rust's NaN guarantees (arithmetic-NaN sign/payload implementation-defined; observed `0/0`→`0xffc00000` is observation, not guarantee); evidence doc still told the withdrawn-narrowing story. Commit `eb6b678`. |
| R5 | 1 P2 | Fixed-binary NaN promise equally unsupported — Rust permits NaN bits to differ between repetitions in one binary. All NaN promises withdrawn; finite-domain scope (inputs+intermediates+results). Commit `6821269`. |
| R6 | 1 P2 | Qualifier not uniform: module docs + three impl attestations still unconditional. Commit `c3a2a95`. |
| R7 | 1 P2 | The PUBLIC mod.rs still unconditional; also removed the fictional "disabled optimizations" mechanism. Commit `a8b1a0b`. |
| R8 | 1 P2 | DetF32 docs: unsupported "10–20% slower" figure + nonexistent `fast` feature. Commit `3975c70`. |
| R9 | **CONVERGED (0/0)** | Full-family sweep verified; all three SIMD rows re-verified green. |

## Contract loop record (6 rounds, cap 12)

R1–R6 converged: measured surface S1–S8 + exclusions (24 verified
unsafe-bearing lines, dispositioned in the archived executable transcript
`census-transcript.txt`); FMA precondition corrected in the contract itself
(R1); census completeness (root `src/deterministic/`, GlobalAlloc proxy,
discovery test fixtures); `rg -E` is `--encoding` not regex; the
cancellation-aware tolerance (ulp-of-result bound disproven at exact ~2⁻⁵⁴
with FMA-exact −5.551115123125783e-17 vs separate-arithmetic 0) with
saturation `M̂ = min(M, f64::MAX)`; exactly 3 `Deterministic` impls. D1–D6
operator-approved (ask_user, all as recommended): D1 dead `aligned_alloc` +
`AlignedBuffer` removed (zero users); D2 real `Clone` replacing `ptr::read`
hacks; D3 `is_multiple_of(8)` batch assert; D4 `transmute_copy` sites kept +
documented; D5 two-tier battery policy frozen; D6 dispatcher architecture
kept, predicate repaired.

## Post-convergence integration (not remediation of reviewed content)

- `2e720bf`: `clippy::missing_safety_doc` — CI runs `--all-features`; the
  feature-gated `deterministic` trait needed a literal `# Safety` section
  (contract text unchanged, moved under the heading + implementor
  obligations). Local rows verified with the feature on: 35/35 tests.
- `bc3aa9b`: develop merge (#315 + #316 Task 35) + catalog regen from the
  merged superset per standing doctrine; manifest hash `752e23d2`; drift gate
  `generated_equals_checked_in` green.

## Final gate state (branch tip `bc3aa9b`, stable 1.98.0)

Battery execution row (+avx2,+fma) 10/10 — 64 basis pairs, adversarial
subnormal/±MAX sets, 10⁴ seeded dense, EXPECTED-DIVERGENT-OVERFLOW class
(FMA exact-intermediate legitimately changes finiteness class in the
saturation regime: scalar NaN vs SIMD signed-inf), bound-rejection +
subnormal regressions, batch contract, name-based runtime inventory
(fn-pointer identity is NOT stable across CGUs — the honest inventory API
`select_geometric_product_impl_name()` exists because of this). Exclusion row
(+avx2) 3/3 — kernel compiled out, scalar fallback reported. Production row
(−avx2) 514/0. Clippy all three rows; fmt; MSRV 1.89 under `+avx2,+fma`;
**Miri PASSED** on the oracle (1756 s) and the kernel (37.7 s — gathers
supported); catalog regenerated, drift gate green; calculus 67/0; root 9/0
(+35/0 with `deterministic`); rewrite rows green via CI.

## Residuals (carried, not blocking)

1. Approximation accuracy uncertified — the contract is determinism, not
   accuracy; Newton-sqrt convergence and Taylor trig bounds predate the
   audit. Documented follow-up (sqrt accuracy improvement noted in the
   contract).
2. ARM never tested (no aarch64 host); the no_std/ARM arm is
   compile-verified only.
3. Reviewer did not independently rerun Miri/MSRV/benches; those rows are
   orchestrator-attested from local runs.
4. C3/C4 undischarged by design (standing).
5. Local `--all-features` clippy blocked by missing cmake for z3-sys
   (environment; CI has cmake) — now worked around by feature-scoped rows.

## Lessons (instances 4–6 of the standing set)

- **Doc claims need code counterparts.** The `deterministic` module carried
  years of fiction — a nonexistent `fast` feature, an unverified overhead
  figure, a phantom "disabled optimizations" mechanism, a false sqrt bound —
  each found only by line-level review of independently readable doc
  surfaces.
- **My own premises need the same verification as code.** The libm
  assumption behind R1's narrowing was never checked against `approx.rs` for
  three rounds; the trait was narrowed on a false premise, then restored.
- **Test the test.** The battery needs rejection regressions proving it
  FAILS on bogus results (the R1 bound-rejection rows exist because the
  saturated bound was once vacuous and green).
