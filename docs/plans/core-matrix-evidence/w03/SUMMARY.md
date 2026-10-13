# CORE-W03 evidence — general inverse and exponential domains

Branch `fix/core-w03-inverse-exp` from develop `9633b69` (post-#287). Contract:
`docs/plans/2026-10-10-amari-core-w03-inverse-exp-contract.md` — 7-round review
loop (R1 2/4/0 → R2 2/2/1 → R3 3/1/0 → R4 2/0/0 → R5 1/1/1 → R6 0/1/1 →
R7 0/0/1-resolved), approved by operator; **one pre-implementation erratum
disclosed in-contract** (3c geometric-product leftover superseded by the exact
route throughout; former Err case `1e200·(e12+e13)` Cl(3,0,0) becomes a
closed-form Ok).

Locks: initial branch lock `53f726d0…`; **regenerated at stage 1 to
`83cedd17…`** by the addition of `num-bigint = "0.4"` (0.4.8, already in the
workspace graph via other crates) as an amari-core dev-dependency for the
exact-square oracle — the only manifest/dependency change on the branch.

## Red (archived: red-inverse.log, red-full.log)

- Inverse behavior-red: 3/3 fail on current develop (non-versor wrong value;
  `1e-8`-scale rejection via `1e-14`; scalar `1e200` → `rev/inf` → zero).
- Exponential API-red: 19 exp/pin items fail to compile (`ExpError` absent).
- Item 8 (`exp(0)`, scalar `f64::exp`) is a pin, not a red — contract
  inaccuracy noted here (the red list's "each fails" overclaims for this item).
- The crafted-dyadic constants were derived exactly (Python `Fraction`); the
  orchestrator initially transcribed the angle discriminant wrong
  (`0x3FDD…` vs correct `0x3FEDEFD99A5D8E5B`) — caught by the stage-1 review,
  fixed, and bit-verified against the float value.

## Stages

- **Stage 1 — exact dyadic square** (`exact_square.rs`, 880 lines): 66-word
  two's-complement accumulator; η table incl. degenerate `0`; ALL `B²` output
  coefficients exactly; RNE+sticky top-128 extraction with `E = 2q + r`
  composition; single-blade bit-exact short-circuit. Unit tests 210/210 incl.
  num-bigint oracle, 4196-bit capacity case, extraction pins. Deviations
  disclosed by the dispatch: `--all-targets` red at this stage by design
  (API-red file), `ExpError` non-`Eq` (f64 fields), `+inf` overflow signal,
  grade-4 zero-flag-only retention (single reused accumulator), temporary
  dead-code allow.
- **Stage 2 — exp routing + series**: frozen precedence implemented
  (zero → scalar `f64::exp` → exact-square routing → fallible series);
  `1e-10`/`1e-14` tests deleted; staged power-of-two scaling, `2^-36` vs
  term-peak, 64-term cap, division-not-reciprocal. 19/22 green (3 inverse red,
  stage boundary). Rotor kept total via a documented bridge (stage-4 scope).
  **Calibration** (long-angle trio `0.1+t·e12`): 1.75e-9 / 1.29e-4 / 3.06e-1
  → **C = 2e4** covers all under `max(1e-6, C·S·u)`. **Superseded by the
  round-2 scalar factoring** (the trio now routes through the closed form; see
  the round-2 section).
- **Stage 3 — LU inverse**: `inverse()` = finite-guarded fast path + LU with
  partial pivoting (computed-pivot policy, honest rustdoc); `1e-14` deleted;
  acceptance battery `inverse_contracts_green.rs` 11/11 (scalar/vector/versor/
  non-versor across Cl(2,0,0)/Cl(2,1,0)/Cl(1,1,1)/Cl(1,0,1), both-order sound
  denominator, nilpotent/null/zero `None` cases). 22/22 contract green.
  Disclosure: the stage-3 dispatch report was truncated mid-sentence; all
  claims were re-verified directly by the orchestrator (22/22, 11/11, 210
  lib, clippy 0, fmt, all-targets 347/0).
- **Stage 4 — consumer migration**: workspace `--all-targets` check exit 0.
  rotor.rs bridge REMOVED; `from_bivector`/`from_multivector_bivector`/
  `from_axis_angle`/`power` → `Result<_, ExpError>` (38 test sites + 3 bench +
  2 example migrated mechanically); wasm `dispatch_exp` propagates through all
  84 generated arms and `generic.rs` maps to a JS error; `LorentzRotor::boost`
  uses a justified expect (finite-rapidity pure bivector, existing panic
  guard). Expect-justification categories recorded (bounded Euclidean
  bivectors; finite-rapidity; bounded log×rational). Crates with only
  `f64`/`Dual` `.exp()` calls verified by the workspace check, not assumed.
  Standalone `examples/rust/*` (non-workspace, pre-existing broken manifests)
  untouched — disclosed.

## Green matrix (stage 5, archived stage5-*.log)

- amari-core: **347 tests / 13 suites on stable AND nightly, identical
  inventories**; clippy `-D warnings` 0 both; no-default-features lib check 0;
  rustdoc `-D warnings` 0; fmt clean.
- Test lineage: 306 (W02 baseline) → 347 (+22 inverse_exp_contracts −0,
  +11 inverse_contracts_green, +8 exact_square unit).
- Workspace tests: exit 101 with exactly the known 24 `probes::supervisor`
  environment-local failures (identical set proven against develop in CAT1).
- **Catalog regenerated from the split layout: `751756ba…`, 11,100 items
  (+4 surface items) — only `crates/amari-core.json` + `manifest.json`
  changed.** First production demonstration of the CAT1 collision-isolation
  property: concurrent rewrite work on develop touches disjoint files.

## Round-2 review remediation (1 P1 / 1 P2 executed)

Findings: (P1) `0.1 + 1e8·(e1+e2)` Cl(1,1,0) returned ~8% vector error because
its non-scalar square carries grade-1 cross terms and the general series
squaring corrupted the scalar channel below the detector threshold; (P2)
`1e10·(e1+e23)` Cl(0,0,3) is a legitimate index-3 nilpotent (`x² = 2e20·e123`,
`x³ = 0`, norm 2.5e19 → 1e20) falsely rejected by the squaring detector, whose
threshold arithmetic `prev·prev` also overflows at large scale.

Three structural fixes, all landed:

1. **Scalar factoring (central decomposition).** `exp(s + y) = exp(s)·exp(y)`
   (scalars are central). Every downstream route runs on the non-scalar part
   `y`; the result is multiplied by `f64::exp(s)` coefficientwise (non-finite
   `exp(s)` → `ScalarRoute`). Routed precedence: zero → pure scalar → factor
   `s`/`y`; on `y`: zero → exact scalar square → exact nilpotent (index ≤ 16)
   → bounded series. Makes scalar-plus-square-zero exact and stops every other
   route from spending squarings on the scalar direction.
2. **Exact nilpotency of any index.** `exact_square.rs` now keeps exact powers
   as per-coefficient dyadic bignums (`66`-word magnitude + `i32` exponent) and
   multiplies them with a schoolbook `66×66` `u64` product (`mul_exact`). It
   iterates `y^(j+1) = y^j·y` to the all-zero certificate or a 16-power cap;
   for a nilpotent of index `d` it returns `Σ_{j<d} y^j/j!` with each exact
   coefficient converted RNE to `f64` and divided by the exact integer
   factorial. Exact for the whole nilpotent class at any scale; a missing
   certificate (capacity/exponent overflow, or `y^d ≠ 0` within the cap)
   falls through to the series instead of claiming a wrong answer.
3. **Overflow-safe detector.** The squaring-cancellation predicate is now
   `new_norm/previous_norm < previous_norm·2^-62` (equivalent for
   `previous_norm > 0`), so the threshold stays finite for every finite
   previous norm; the old `previous_norm²·2^-62` overflowed to `+inf` above
   `2^512`. The detector remains a conservative heuristic — with nilpotents
   routed exactly its residual false-positive class is non-nilpotent
   sub-quadratic growth, and it errs honestly (`NonFiniteResult { Squaring }`)
   rather than returning a silently wrong value (documented in rustdoc).

Deviations resolved (reported in the dispatch):

- The plan asked for an index-4 case in Cl(0,0,4). That signature has **no**
  nilpotency index above 3 — verified exhaustively over all `0/1` element
  subsets (2^15) and by random integer search. The faithful closest case is the
  index-4 element `x = e1 + e23 + e45` in Cl(0,0,5) (`x² = 2(e123+e145+e2345)`,
  `x³ = 6e12345`, `x⁴ = 0`), asserted against an independent repeated-
  multiplication `f64` evaluation.
- The overflow-safe detector-threshold check is a unit test of the private
  `squaring_is_collapse` helper, so it lives inline in `amari-core/src/lib.rs`
  rather than in `inverse_exp_contracts.rs` (an integration test cannot reach
  a private item).

Recalibrated long-angle trio (`0.1 + t·e12`, Cl(2,0,0)): the class is now
**closed form** (`y = t·e12` has the exact scalar square `-t²`), so the former
series-route numbers no longer describe it. Achieved relative error against
`e^0.1·(cos t + sin t·e12)` is **0 (bit-exact)** for `t ∈ {1e4, 1e8, 1e12}` —
the angle is `|t|` bit-for-bit via the single-blade short-circuit, so the only
operations are the common `cos`/`sin` and the shared `e^0.1` scaling.

Round-2 evidence (archived `stage-r2-*.log`): amari-core lib **216/216**
(212 prior + 3 exact-power oracle unit tests + 1 detector unit test);
`inverse_exp_contracts` **32/32** (28 prior + 4 round-2 regressions);
`inverse_contracts_green` **11/11**; workspace `--all-targets` check exit 0;
clippy `-p amari-core --all-targets -D warnings` 0; rustdoc `-D warnings` 0;
fmt clean.

## Resolved-by-contract numerical facts worth restating

- `exp` of exact nilpotents is exact up to one final rounding per
  coefficient: the nilpotency **certificate** and the polynomial
  accumulation are exact integer arithmetic, and the single
  round-to-nearest-even conversion of `N/16!` is exact wherever the exact
  rational value is representable (and correctly rounded otherwise). Round 1
  gave `1+B` for the square-zero class; round 2 extended it to **any
  nilpotency index ≤ 16**; round 3 replaced the per-term `f64` conversion by
  the common-denominator exact accumulator (see the round-3 section), which is
  what makes near-cancelling coefficients exact.
- Degenerate signatures route through η = 0 (Cl(1,0,1) etc.).
- Mixed-signature planes use the exact scalar square (√3, not √5).
- **The huge-scale regime (`1e200` class) is correct where representable for
  the certified exact classes only — scalar-square and nilpotent inputs, which
  are computed by exact arithmetic before any squaring.** Non-certified
  general multivectors at extreme scales do not promise that regime: they route
  to the bounded series and error honestly (`NonFiniteResult`) rather than
  returning a silently wrong value. The residual false-positive class of the
  detector is non-nilpotent sub-quadratic growth, as documented in `exp`.

## Round-3 review remediation (2 P1 / 1 P2 / 1 P3 executed)

Findings, all reproduced by the reviewer and fixed here by **completing** the
round-2 exact architecture rather than changing its design:

1. **P1-1 — scalar-factoring underflow.** `x = -750 + 1e300·(e1+e2)` in
   Cl(1,1,0) returned all-zero: `f64::exp(-750)` underflows to `0` before the
   `1e300` coefficients are multiplied. The scalar factor is now carried
   **symbolically** as `exp(s) = exp(s - E·ln2)·2^E`, where `E` is the nearest
   integer to `s/ln2` and `s - E·ln2` is reduced with the fdlibm `ln2 = LN2_HI
   + LN2_LO` split (`E·LN2_HI` is exact for `|E| < 2^21`, and the subtraction
   is exact by Sterbenz). The mantissa is one `f64::exp` (~1 ulp) and the
   integer exponent is applied per coefficient with a single `ldexp`-style
   RNE step (`scale_by_pow2_rne`). The naive `2^(s·log2 e)` decomposition with
   a single rounded `LOG2_E` produces ~7 ulp and is not used. The `-750` vector
   coefficients are now bit-exact `1.9016849634750064e-26`; the `+750`
   overflow side is an honest `ScalarRoute` error because the result's scalar
   coefficient is `exp(750)` itself (mathematically `> f64::MAX`).
2. **P1-2 — polynomial accumulation rounding.** `x = a·e1 + a·e23 - a²·e123`
   with `a = nextafter(1e10)` in Cl(0,0,3) has exp e123 coefficient
   `x[7] + a² = 1478560745259009/274877906944 = 5378.972656250004`; the old
   per-term `f64` conversion plus division cancelled the residual to `0`. The
   polynomial is now accumulated **coefficientwise in exact integer
   arithmetic**: `N_blade = Σ_j y^j[blade]·(16!/j!)` (exact bignum × small
   integer), and `N_blade/16!` is rounded to `f64` **once**. (`16!` clears
   every `j!`, `j ≤ 16`; the plan text's `lcm(1..d)` does not clear `j!` for
   `j ≥ 4`.) No `f64` exists between certification and the single final
   rounding. Regression pins the exact coefficient within `1e-12` relative and
   a `num-bigint` rational **D-scheme oracle** pins it bit-for-bit.
3. **P2 — certification/evaluation separation.** The certification power
   iteration now stays entirely in bignum and never converts; reaching the
   16-power cap without a zero certificate (or an exact power/accumulator
   overflow) falls through to the residual series + detector, and **never**
   reports the certification stage. Only a **certified** polynomial whose
   final post-division coefficient is non-finite reports
   `NonFiniteResult { stage: ClosedForm }`. (a) `1e154·(e1+e23)` in Cl(0,0,3)
   now returns `Ok` with e123 exactly `1e308`, where the raw `x²` coefficient
   `2e308` overflows `f64` and the old code errored. (b) `0.1 +
   1e80·(e12+e3)` in Cl(2,0,1) no longer reports `ClosedForm`; the certified
   path falls through to the series and the round-1 detector fires honestly
   (`Squaring`) — see the deviation note below.
4. **P3 — false index bound.** The "index ≤ 7 for dim ≤ 6" comment was false:
   the pinned Cl(3,3,0) element below has index 8 (`x⁷ ≠ 0`, `x⁸ = 0`). The
   comment now states the semisimple-quotient/radical bound ≤ 12 for dim ≤ 6
   and documents the cap 16 as `≥` that bound with margin; the index-8 input is
   a regression test and is also validated against an independent `f64`
   repeated-multiplication Taylor evaluation.

Round-3 evidence (archived `stage-r3-*.log`): amari-core lib **218/218**
(216 prior + the D-scheme oracle + the symbolic-scale composition unit test);
`inverse_exp_contracts` **38/38** (32 prior + 6 round-3 regressions);
`inverse_contracts_green` **11/11**; workspace `--all-targets` check exit 0;
clippy `-p amari-core --all-targets -D warnings` 0; rustdoc `-D warnings` 0;
fmt clean.

Deviation (established by the orchestrator's direct verification — the round-3
remediation dispatch exited without a report, so the orchestrator re-ran and
confirmed the gates and probes itself): the P2b regression was enumerated as
`→ Ok` against `e^0.1·(cos t + sin t·e12)·(1 + t·e3)`. The input is not a
scalar-square and not nilpotent, so the only available route is the bounded
scaling-and-squaring series. With the frozen series rules the scaled-domain
truncation error (`2^-36`) is amplified by `2^k` (`k ≈ 268`) during the 268
squarings, the result collapses, and the round-1 squaring detector fires — an
honest **evaluation-path** error (`Squaring`), not the certification error. A
finite `Ok` is not reachable without changing the frozen series rules or adding
a new decomposition route (both excluded). The regression therefore pins the
required behavior — never `ClosedForm` — and, if the series ever returns `Ok`,
checks it against the factorization within the condition-aware bound
`max(1e-6, 2e4·S·u)`; the route is documented as series + detector.

## Round-4 review remediation (3 P1 / 1 P2 / 1 P3 executed)

**Unifying root cause:** premature `f64` materialization before the symbolic
scalar scale was composed. Every round-4 failure was a point where an
intermediate was rounded to `f64` (or formed a full-scale product) before the
integer exponent `2^E` could rescue it, or before the true destination
(subnormal) precision was known. The fix is therefore **one pipeline**, not
three patches: a single correctly-rounded "exact integer times `2^e`, rounded
exactly once at the destination precision" primitive, shared by the symbolic
scalar composition and the exact-polynomial finalization, with the symbolic
scale carried **into** the closed forms and the polynomial rather than applied
after them.

### The unified primitives (`exact_square.rs`)

- `round_words_scaled_to_f64(negative, words, e_offset, external_sticky)` —
the one rounding decision. It normalizes the exact integer, determines the
destination precision (`53` bits normal, the true subnormal precision
otherwise), takes one round-to-nearest-even decision with the division remainder
folded into the sticky bit, and composes the integer exponent exactly. No
intermediate normalized-then-subnormal rounding exists.
- `apply_symbolic_scale(c, mantissa, exponent)` — decomposes `c` and the scale
mantissa, forms the exact `53 x 53 -> 106`-bit mantissa product, and calls the
primitive once. It never forms `c * mantissa` in `f64` first.
- `wide_rational_scaled_to_f64(magnitude, exponent, denominator, negative,
scale_mantissa, scale_exponent)` — folds the scale mantissa exactly into the
division numerator, the scale exponent into the final exponent, and the
division remainder into the sticky bit, then calls the primitive once.

### Findings, all landed

1. **P1-1 — symbolic composition overflow / subnormal order.** (a)
   `-0.5 + f64::MAX·(e1+e2)` in Cl(1,1,0) previously formed
   `f64::MAX · mantissa` (`> f64::MAX`), producing `+inf` before the `-1`
   exponent could rescue it. `apply_symbolic_scale` returns the finite
   `[0.6065306597126334, 1.0903560030489124e308, 1.0903560030489124e308, 0]`.
   (b) `0.4 + 2^-1074·e1` in Cl(0,0,1) previously rounded the subnormal
   mantissa product *before* the `+1` shift (double rounding) and returned
   `from_bits(2)`; the single destination-precision composition returns
   `from_bits(1)`.
2. **P1-2 — certified routes rejected finite finals.**
   (a) `-750 + 750·e1` in Cl(1,0,0): `cosh(750)`/`sinh(750)` overflow, but
   `e^-750·cosh(750) = (1 + e^-1500)/2` is finite. The hyperbolic branch now
   evaluates the symbolic exponential terms `(e^{s+a} ± e^{s-a})/(2 or 2a)`
   instead of `cosh`/`sinh`, returning `[0.5, 0.5]`.
   (b) `-750 + 1e200·(e1+e23)` in Cl(0,0,3) is a certified index-3 nilpotent
   whose unscaled e123 coefficient (`~1e400`) overflows `f64`; the symbolic
   scale is now composed into the single final division rounding, returning
   scalar `0`, e1/e23 `= 1.9016849634750064e-126`, e123 `=`
   `1.9016849634750064e74` (within 1 ulp — the symbolic mantissa is within
   1 ulp of true `e^-750`).
3. **P1-3 — double-rounded subnormal division.** For the crafted
   Cl(0,0,3) input, the exact e123 coefficient `(5/2 + 2^-52)·2^-1074` is
   strictly above the subnormal midpoint; the single destination-precision
   rounding with the quotient remainder as sticky returns `from_bits(3)`
   where the old normalized-then-subnormal path returned `from_bits(2)`.
4. **P2 — clippy `needless_range_loop` x2.** The `mul_exact` double loop now
   uses `a.coeffs.iter().enumerate().take(basis_count)` and the corresponding
   `b.coeffs` iterator. Clippy `-D warnings` is 0 (log header records the
   toolchain).
5. **P3 — SUMMARY misattribution.** The round-3 deviation paragraph now
   states that the orchestrator established it by direct verification (the
   round-3 dispatch exited without a report), rather than “reported in the
   dispatch”.

### Symbolic-term-sum absorption rule

`cosh`/`sinh` overflow only when `a >~ 710`, and then the two exponential
terms differ by `e^{2a} > 2^2048 > 2^53`, so the smaller term is dominated by
more than 53 bits (relative weight below `2^-984`) and is **absorbed**: the
dominant term `e^{s+a}/2` (or `e^{s-a}/2`) alone determines the rounded scalar,
and `dominant_mantissa/(2a)` scaled by `2^{dominant_exponent}` gives the vector
coefficients. If the dominant term is itself not representable the result is an
honest `ClosedForm` non-finite error. For the finite regime (bounded `cosh` and
`sinh`) and the circular/square-zero classes the scale is applied through
`apply_symbolic_scale`; a non-finite product whose *unscaled* closed form was
finite is reported as `ScalarRoute`, preserving the round-3 stage distinction.

### Round-4 evidence (archived `stage-r4-*.log`; toolchain in the log headers)

- amari-core lib **220/220** (218 prior + 2 new scaler unit tests:
  `symbolic_scale_overflow_composition_is_finite`,
  `symbolic_scale_subnormal_order_rounds_once`).
- `inverse_exp_contracts` **43/43** (38 prior + 5 round-4 regressions:
  `round4_symbolic_scale_overflow_composition_is_finite`,
  `round4_symbolic_scale_subnormal_order_is_correct`,
  `round4_hyperbolic_closed_form_carries_symbolic_scale`,
  `round4_nilpotent_finalization_carries_symbolic_scale`,
  `round4_subnormal_division_rounds_once_at_destination`).
- `inverse_contracts_green` **11/11**; workspace `--all-targets` check exit 0;
  clippy `-p amari-core --all-targets -D warnings` 0; rustdoc `-D warnings` 0;
  fmt clean.

Deviation: the P1-2b e123 pin carries a 1 ulp tolerance. The task reference
`1.9016849634750064e74` is the correctly-rounded value of `(1e200)^2 · e^-750`
for the *true* `e^-750`, while the implementation composes the single rounded
`f64` mantissa of `e^-750`, which is 1 ulp below true; the result therefore
rounds 1 ulp below the true-value reference. This is the documented ~1 ulp
symbolic-scale accuracy, and matches the existing round-3 convention
(`round3_scalar_factoring_rescues_underflowed_scale` uses the same tolerance).
No other round-4 repro needs a tolerance.

## Round-5 review remediation (2 P1 / 1 P2 executed)

**Reviewer validation context:** the independent reviewer verified the rounding
pipeline primitives with **20,000 exact-`Fraction` comparisons** and **3,000
public subnormal nilpotent cases** — **zero** mismatches. The round-5 findings
are therefore not pipeline defects but **call-site leaks** of the validated
pipeline plus one stage-classification gap. No architecture changed: the
existing primitives were extended.

### Extensions (`exact_square.rs`)

- `apply_symbolic_scale3(c, factor, scale_mantissa, scale_exponent)` — the
  three-way symbolic multiply for circular/finite-hyperbolic vector
  coefficients. The coefficient, the `sinc`/`sinhc` factor, and the symbolic
  scalar scale are decomposed to exact integer mantissas and binary exponents;
  their exact `53 × 53 × 53 → 159`-bit product is formed and a **single** RNE
  decision is taken at the destination precision with the integer exponent
  composed exactly. It never forms `c · factor` (or `c · scale_mantissa`) in
  `f64` first.
- `apply_symbolic_scale_over_2a(c, angle, scale_mantissa, scale_exponent)` — the
  hyperbolic symbolic fallback's vector coefficient
  `e^{s+a}/(2a)·c`. The divisor `2a` is never formed as an `f64` (it overflows
  for `a > MAX/2`): `angle = m_a · 2^{e_a}` is decomposed and the exact rational
  `1/(2a) = 1/(2·m_a) · 2^{-e_a}` is folded into the single destination
  rounding, so `c = a` reproduces the scalar `e^{s+a}/2` **bit-for-bit**.
- `NilpotentOutcome` now distinguishes `NonFiniteClosedForm` (the *unscaled*
  coefficient is unrepresentable) from `NonFiniteScalarRoute` (the unscaled
  coefficient is finite; only the symbolic scale pushed it out of range).
  `finalize_nilpotent` re-rounds a non-finite result without the scale to make
  the distinction; `exp_nonscalar` maps the two to `ClosedForm` and
  `ScalarRoute` respectively.

### Findings, all landed

1. **P1-1 — circular/finite-hyperbolic coefficient scaling materialized
   `c·sinc`/`c·sinhc` in `f64` first.** (a) Cl(2,1,0)
   `x = -1 + e1 + MAX·(e2+e3)` has the exact nonscalar square `+1`; the old
   path formed `MAX · sinhc(1) > MAX` and errored before `e^-1` could rescue
   the finite `e2/e3 = 7.77200912641605e307`. (b) Cl(1,2,0)
   `x = 700 + 1e-300·e1 + 1e100·e2 + 1e-300·e3` has the exact scalar square
   `-(1e100)^2`; the old path formed `1e-300 · sinc` (underflow to 0) before
   `e^700` could rescue `e1/e3 = -3.8605498802690016e-97`. `exp_scalar_square`
   now returns the scalar channel through `apply_symbolic_scale` and each vector
   channel through `apply_symbolic_scale3`.
2. **P1-2 — hyperbolic symbolic fallback bypassed unified rounding.** (a)
   Cl(1,0,0) `x = -MAX + MAX·e1` previously computed `2.0 * angle` as `+inf`
   and collapsed the vector coefficient to `0`; it now returns `[0.5, 0.5]`
   exactly. (b) `s = -5926.141552874991, a = 5214.592735436862` previously
   double-rounded the vector channel to bits `96263207714728`; both channels are
   now `96263207714727` (`4.75603438903267e-310`).
3. **P2 — polynomial scale-overflow misclassified as `ClosedForm`.** Cl(0,0,3)
   `x = 700 + 1000·(e1+e23)` has finite unscaled polynomial coefficients
   `1, 1000, 1000, 1e6`; only the `e^700` scale pushes `e123` out of range.
   The exact-nilpotent finalization now reports `NonFiniteScalarRoute`
   (`Err(NonFiniteResult { stage: ScalarRoute })`), honouring the round-4 stage
   distinction, instead of `ClosedForm`.

### Regressions (all five round-5 repros pinned)

`round5_hyperbolic_factor_scaling_is_symbolic` (P1-1a),
`round5_circular_factor_scaling_is_symbolic` (P1-1b),
`round5_hyperbolic_symbolic_fallback_never_forms_two_a` (P1-2a),
`round5_hyperbolic_symbolic_vector_matches_scalar_bits` (P1-2b, bit equality),
`round5_polynomial_scale_overflow_reports_scalar_route` (P2, asserts the
`ScalarRoute` variant specifically).

### Round-5 evidence (archived `stage-r5-*.log`; toolchain in the log headers)

- amari-core lib **222/222** (220 prior + 2 new scaler unit tests:
  `three_way_symbolic_scale_composes_before_rounding`,
  `symbolic_scale_over_2a_is_exactly_half_for_c_equals_a`).
- `inverse_exp_contracts` **48/48** (43 prior + 5 round-5 regressions).
- `inverse_contracts_green` **11/11**; `cargo check --workspace --all-targets`
  exit 0; `cargo fmt --all -- --check` clean; clippy
  `-p amari-core --all-targets -D warnings` 0; rustdoc `-D warnings` 0.

No tolerance was needed on any round-5 pin: P1-1a/P1-1b match their exact
references bit-for-bit, P1-2a is exactly `[0.5, 0.5]`, and P1-2b matches bits
`96263207714727` on both channels.

## Review loop — convergence record

R1 2/1/0 → R2 1/1/0 → R3 2/1/1 → R4 3/1/1 → R5 2/1/0 → R6 **0/0/1** (converged at the 6-round cap; independent final verdict "ready to commit and open a PR").

The loop drove five remediation waves beyond the staged implementation:
1. R1: generalized exact square to all multivectors; scalar-square routing; detector; fast-path finite quotient; k-adjust.
2. R2: central scalar factoring (exp(x) = e^s·exp(y)); exact nilpotency of any index (bignum×bignum powers, Σ y^j/j!); overflow-safe detector.
3. R3: symbolic scalar-exponential scaling (2^E·2^f); exact-rational polynomial accumulation (lcm denominator, single final RNE); certification/evaluation separation (no f64 in the power search; cap-exceeded falls through to the residual route); justified index bound (≤12 for dim ≤ 6, cap 16; Cl(3,3,0) index-8 pinned).
4. R4: the unified finalization pipeline (round_words_scaled_to_f64 / apply_symbolic_scale / wide_rational_scaled_to_f64 — single destination-precision rounding); symbolic hyperbolic terms with the absorption rule; ScalarRoute/ClosedForm stage split.
5. R5: three-way symbolic multiply (scale3) and exact-rational 1/(2·m_a) for the absorbed hyperbolic vector channel; NilpotentOutcome stage split.

Reviewer validation totals: 20,000 + 3,000 (R4 primitives) + 50,000 (25k per R5 primitive) exact-Fraction comparisons, 6,000 degenerate-signature probes — zero mismatches. Every certified class now computes in exact or single-rounded arithmetic end-to-end; the residual uncertified class errs honestly (documented conservative detector).

## Final gates (post-P3-doc-fix)

- amari-core: **385 tests / 13 suites on stable AND nightly, identical inventories**; clippy 0 both; workspace all-targets check 0; fmt clean. Test lineage: 306 (W02) → 347 (stages 1–4) → 385 (review-loop regressions + unit tests).
- Catalog regenerated from the split layout: `9d480608…`, 11,100 items — only `crates/amari-core.json` + `manifest.json` changed (CAT1 isolation holding).
- Lock `83cedd17…` (num-bigint dev-dep; regenerated at stage 1, stable since).
- Residual disclosed limitations: conservative residual-route rejection (finite-representable inputs of the commuting-factorization class return Err); randomized verification is evidence, not proof; "single rounding" composes already-rounded transcendental factors.
