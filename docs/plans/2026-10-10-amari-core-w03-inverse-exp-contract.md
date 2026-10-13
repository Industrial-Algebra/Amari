# CORE-W03 — General inverse and exponential domains (CORE-01, 03–06 / D02)

**Status:** ready for operator approval (rounds 1–7 complete; round-6 amendments verified by round 7: 0 P1 / 0 P2)
**Base:** develop `9633b69` (post-#287; contains W02 + catalog split + toml 1.1)
**Decisions:** D02.1–D02.4 settled by operator 2026-10-10. Amendments below correct
numerical policy within those decisions; they do not reopen them.

## Scope

`amari-core` `inverse()`, `exp()`, `exp_series()` (lib.rs:564–629 region) plus every
workspace consumer of the changed signatures, **including** `rotor.rs` constructors
(lines 20–38) and `power()` (line 276), and the generated wasm dispatch
(`amari-wasm/build.rs:231–243` unary arms, `amari-wasm/src/generic.rs:444–453`
wrapper). **Breaking by design** (`exp()` becomes `Result`-returning; ~129 exp and
~28 inverse call sites). Mathematical correctness is paramount; compatibility is not
a constraint.

## D02.1 — inverse(): general LU + fast exact path

- Left-multiplication matrix of `M` (column j = coefficients of `M·e_j`;
  N = 2^(p+q+r)); a right inverse from solving `M·X = 1` is automatically the left
  inverse in this finite-dimensional unital algebra.
- `inverse()` = LU with partial pivoting. **Domain rule (computed-pivot policy):**
  `None` iff any post-pivot pivot is exactly zero, or any elimination/solution step
  produces a non-finite value. This is a numerical acceptance rule, not an
  exact-arithmetic invertibility certificate: rounding can erase a mathematically
  nonzero pivot (near-singular inputs such as `1+e1+1e-9·e2` in Cl(2,0,0)) or admit
  an ill-conditioned solve. No `Some`-guarantee for every mathematically invertible
  f64 input; conditioning and representability limitations documented in rustdoc.
  Cost: O(N²) storage, O(N³) solve.
- **Fast path** (inside `inverse()`): iff `M·rev(M)` is a computed nonzero
  **finite** scalar (non-scalar coefficient norm exactly zero — computed caveat —
  and finite nonzero scalar part), return `rev(M) / λ` (division, never reciprocal
  multiply). Computed-scalar membership is not a certificate under overflow,
  underflow, or cancellation (scalar `1e200` → λ = inf → routes to LU →
  representable `1e-200`).
- Closes CORE-01 and the `1e-14` domain errors.

## D02.2 — threshold semantics

- Structural membership tests are exact zero tests **on computed values**, with the
  overflow/cancellation caveat stated wherever the tested quantity arises from
  products.
- Numeric comparisons scale-relative to operand `coefficient_norm`.
- All scaling by division; never `* (1.0/λ)`.

## D02.3 — exp(): exact-domain closed form + series fallback

Returns `Result<Self, ExpError>`:

```rust
pub enum ExpError {
    NoConvergence { terms: usize, last_term_norm: f64, result_scale: f64 },
    NonFiniteResult { stage: NonFiniteStage },
}
pub enum NonFiniteStage { ScalarRoute, ClosedForm, SeriesScaling,
                          SeriesAccumulation, Squaring }
```

**Routing precedence (frozen, in order):**

1. `is_zero()` → exactly `Self::scalar(1.0)`, `Ok`.
2. Exact pure scalar `s` → `Self::scalar(f64::exp(s))`, `Ok`; non-finite →
   `Err(NonFiniteResult { stage: ScalarRoute })`.
3. Exact bivector (non-grade-2 part exactly zero) — **exact dyadic square**:
   every f64 coefficient is a dyadic rational (±m·2^e, m a 53-bit integer), so
   EVERY output coefficient of `B²` (scalar and grade ≥ 4 alike) is a signed sum
   of products of two such values — computable **exactly** with a small
   bounded-sign accumulator over aligned powers of two (each product is a ≤ 106-bit
   mantissa times 2^(e_i+e_j); ≤ 64·64 products — a fixed-size bignum, no
   allocation, no dependency; **proved capacity: 66 u64 words** — the aligned dyadic range spans
   products from 2^-2148 (two minimum subnormals) to just under 2^2048 (two maximum
   finites), i.e. 4196 magnitude bits, plus one sign bit and headroom for ≤ 4096
   accumulated terms — magnitude requires at most 4208 bits, signed
   representation 4209, and 66 words provide 4224 (verified
   against the exact scalar square of `H·e12 + L·e13` in Cl(3,0,0) with
   H = max-finite, L = min-subnormal, whose aligned integer has 4196 bits). The
   bivector square `B²` is therefore evaluated EXACTLY:
   - **Nilpotence certificate:** all exact output coefficients zero ⇒ `B² = 0`
     as a multivector (scalar AND grade ≥ 4 — this is the only sound form;
     vanishing scalar square alone does NOT certify nilpotence:
     `e12 + e34` in Cl(3,1,0) has scalar square exactly 0 but `B² = 2·e1234`,
     true exp `(cos1+sin1·e12)(cosh1+sinh1·e34)` with all four components).
     Exact certificate ∧ `B ≠ 0` → return exactly `1 + B`, `Ok` (closes CORE-03),
     valid at ANY scale — `1e200·(e12+e13)` in Cl(2,1,0) certifies exactly
     (its −1e400 and +1e400 cancel as integers) and returns `1+B` with no series
     and no squaring.
   - **Simplicity:** the exact grade ≥ 4 part of the dyadic square nonzero ⇒
     non-simple → **series** (closes CORE-04); exact zero ⇒ simple. The exact
     dyadic square computes every output coefficient, so no geometric-product
     evaluation (and no non-finite Err route) exists anywhere in routing.
     [ERRATUM 2026-10-12, pre-implementation: the prior draft's 3c
     "geometric product; any non-finite coefficient → Err" was a leftover from
     the round-4 design that contradicted the approved exact-dyadic square; it
     is superseded by the exact route, which turns the former Err case
     (e.g. `1e200·(e12+e13)` in Cl(3,0,0), exact scalar square −2·2^400, grade-4
     zero) into a correct closed-form Ok.]
   - **Branch and magnitude:** sign of the exact scalar sum selects
     circular (negative) / hyperbolic (positive). Magnitude extraction is
     **specified exactly** (never `m²`, never the geometric product, never a
     rounded normalized sum):
     (i) single-blade bivectors short-circuit: angle = `|c|`, the original f64,
     bit-for-bit (no extraction);
     (ii) multi-blade: normalize the exact signed integer to the form
     `|B²| = m · 2^E` with `m ∈ [1, 2)` carried as a top-128-bit working mantissa
     (round-to-nearest-even, remaining bits sticky — the extracted value is
     within 1 ulp of the exact rational). Decompose `E = 2q + r`, `r ∈ {0, 1}`.
     The angle is `sqrt(m · 2^r) · 2^q`: evaluate `sqrt` on the f64-rounded
     `m · 2^r` (IEEE-correctly-rounded; mantissa rounding may carry into the
     exponent — handled), then rescale by `2^q` via exponent arithmetic
     (`ldexp`), with final subnormal rounding to nearest-even and overflow
     mapping to the non-finite policy. Worked cases: `|B²| = 2^-2148` →
     `E = −2148 = 2·(−1074) + 0` → angle `2^-1074` (survives; neither the square
     nor the unscaled integer is ever converted directly to f64);
     `|B²| = 8` (B = 2e12+2e13, Cl(3,0,0)) → `m = 1, E = 3 = 2·1 + 1` →
     `√2 · 2 = 2.8284271247461903` (naive pre-sqrt exponent halving would give
     the wrong 1.414…). Final angle error ≤ ~1 ulp. A truncating
     extraction is non-conforming: 1-ulp-low angles at ~1e200 scale change
     cos/sin materially (round-5 reproduction: extracted angle 1 ulp below the
     exact `0x1.7bde5cfb5d30ep+664` flips the scalar from −0.9866 to −0.8823).
     Note the coefficient norm is NOT the plane magnitude (wrong in mixed
     signatures: `2e12+e13` in Cl(2,1,0): B² = −3, angle √3 vs ‖B‖_coeff = √5).
   - No floating-point certificate is used anywhere in routing: compensated
     summation of rounded terms cannot certify (rounding when forming individual
     terms loses the information — e.g. normalized dyadic terms summing to exactly
     0 while the exact square is ~0.875 with dominant-coefficient error 13%; and
     `1e200·e12 + e13 + 1e200·e23` in Cl(1,2,0) whose exact square is 1 but whose
     rounded normalized terms give a false zero). Uncertainty does not exist in
     this route: the square is exact or the input was not an exact bivector.
4. General multivector → series.

**Exact-square implementation bounds:** the accumulator is a fixed array of
**66 u64 words** (4224 bits ≥ the proved 4209-bit worst case above) with
two's-complement signed accumulation; products come from 64×64→128-bit
multiplication (u128). Cross-validated in tests against num-bigint
(dev-dependency) over exhaustive small blades, randomized coefficients, AND a
deterministic mixed-extreme case (max-finite and min-subnormal coefficients in
the same output sum) that pins the capacity.

The coefficient norm is never used as a plane magnitude (wrong in mixed
signatures: `2e12+e13` in Cl(2,1,0) has B² = −3, angle √3, while ‖B‖_coeff = √5).

Every finite multivector has a convergent exact-arithmetic exponential; f64 failure
modes are representability and roundoff, framed as above.

## D02.4 — exp_series(): bounded, frozen rules

- Private machinery.
- **Scaling:** choose k = smallest integer with scaled coefficient norm ≤ 0.5;
  implement as **staged power-of-two division** (successive divisions by
  representable powers of two, e.g. 2^64 chunks — never constructs an
  unrepresentable divisor; a finite input like `1+1e308·e1` needs k ≈ 1025, where
  `2^k` is not an f64). Exactness: divisions by powers of two are exponent shifts,
  exact while coefficients remain normal; division of a subnormal by 2^64 rounds
  (e.g. `2^-1074 / 2^64 → 0`) — since the scaled target is norm ≈ 0.25–0.5 this
  only affects inputs with extreme coefficient spreads, and the loss is documented
  rather than silently absorbed. Non-finite during scaling → `Err(SeriesScaling)`.
- **Squaring-channel hazard (documented):** repeated squaring evaluates the scalar
  channel through products whose cancellation can erase it at extreme scales (the
  round-3 collapse mechanism). Exact-nilpotent bivectors never reach this route
  (precedence 3, exact certificate); general multivectors at extreme scales carry
  this documented hazard, bounded by the condition-aware accuracy statement below —
  the implementation must not claim more.
- **Loop:** `term_norm > ε_rel · max(result_norm, term_peak_norm)` with
  `ε_rel = 2^-36`, term peak initialized with the identity term; hard cap **64
  terms** (plausible: dimension-64 scaled l1 norm ≤ 4 gives `4^64/64! ≈ 2.7e-51`);
  then k squarings.
- **Accuracy statement (condition-number honest):** the frozen rules guarantee
  small *scaled-domain* truncation error. Repeated squaring amplifies absolute angle
  error by ≈ 2^k; the final relative error is therefore **inherently conditioned on
  the input scale**: for inputs with dominant coefficient scale S, the oscillatory
  accuracy is limited by ≈ S·u (an angle of ~1e12 is only specified to ulp(1e12) ≈
  1.2e-4 by its own f64 representation). The rustdoc states: final relative error
  ≤ max(1e-6, C·S·u) for a documented constant C (empirically calibrated), and the
  API promises nothing tighter for long-scale inputs. No universal empirical bound
  is claimed. Non-convergence at cap → `Err(NoConvergence)`; non-finite →
  `Err(SeriesAccumulation | Squaring)`. No silent truncation (closes CORE-06).

## Consumer migration rules

- `x.inverse()`: semantics unchanged for versors; non-versor invertible elements now
  return `Some` (was: wrong value or `None`).
- `x.exp()` → `x.exp()?` or explicit match; `expect` ONLY where the caller has
  independently established range and structure — never by grade membership alone.
- **Ripple (named obligations):** `rotor.rs:20–38` constructors and `:276 power()`
  become `Result`-returning (error propagation only — rotor/log mathematics stays
  W07); wasm generated dispatch propagates `ExpError` and `generic.rs` maps it to a
  JS error.

## Verification

Red-first regressions (each fails against current develop):

1. Non-versor wrong inverse (CORE-01 audit counterexample).
2. Small-scale inverse rejection (`1e-8`-scale versor → currently `None`).
3. Fast-path overflow guard: scalar `1e200` → LU yields `1e-200`.
4. Nilpotent deletion: nonzero `B` with `B² = 0` → `1 + B` exactly.
5. CORE-05 dispatch defect: `1e-11·(e1+e2)` in Cl(1,1,0) (square-zero vector;
   true exp `1+v`) — currently deleted by the absolute `1e-10` membership test.
6. Degree-19 silent truncation (CORE-06) → converges or errors, never silently
   truncated.
7. Non-simple bivectors: `e12+e34` in Cl(4,0,0) AND in Cl(3,1,0) (scalar square
   zero, grade-4 nonzero — pins the frozen precedence) → series, matches reference.
8. `exp(0) = 1` exactly; scalar `exp(s) = f64::exp(s)`.
9. Range cases: `1e200·e12` (Cl(2,0,0)) bounded-correct via the signature-aware
   angle route; `1000·e12` (Cl(1,1,0)) → `Err(NonFiniteResult)`; `1e200·(e12+e13)`
   (Cl(2,1,0), NaN geometric square, exact square 0) → **exactly `1+B` via the
   exact-nilpotence certificate, no series, no squaring**; `1e200·(e12+e13)`
   in Cl(3,0,0) (exact scalar square −2·2^400, simple, circular) → closed-form
   `Ok` via the exact route [erratum case];
   `1 + 1e308·e1` staged scaling converges or errors honestly.
10. Near-zero circular/hyperbolic generators (tiny nonzero square, both signs).
11. **Mixed-signature multi-component planes:** `2e12+e13` in Cl(2,1,0) (circular,
    angle √3) and a hyperbolic counterpart — closed form matches high-precision
    reference (guards against the coefficient-norm shortcut regression).
12. **Long-angle general multivectors:** `0.1 + t·e12` in Cl(2,0,0) for t ∈
    {1e4, 1e8, 1e12} — acceptance is the condition-aware tolerance above; the
    test freezes the achieved accuracy.
13. **Degenerate signatures:** `exp(e12) = 1 + e12` exactly in Cl(1,0,1) (η = 0
    blade square); a hyperbolic example in a mixed p,q,r signature.
14. **Rounded-term false zero:** `1e8·e12 + e13 + 1e8·e23` in Cl(1,2,0) (exact
    B² = 1) and the round-4 crafted case with dyadic normalized terms summing to
    exactly 0 while the exact square is nonzero (dominant-coefficient error 13%) —
    both route by the EXACT square to the correct hyperbolic closed form (pins the
    exact-arithmetic requirement; a compensated-summation implementation fails
    these).
15. **Scalar-zero non-nilpotent:** `e12 + e34` in Cl(3,1,0) (scalar square 0,
    B² = 2e1234) → series; matches the commuting-factorization reference
    `(cos1+sin1·e12)(cosh1+sinh1·e34)`.
16. **Exact-square oracle:** bignum accumulator cross-validated against
    num-bigint (dev-dep) on exhaustive small-signature bivectors, randomized
    coefficients, AND the deterministic mixed-extreme capacity case
    (max-finite + min-subnormal coefficients in one output sum).
17. **Magnitude-extraction pins:** the round-5 huge-angle reproduction class
    (`0x1.7bde5cfb5d30ep+664`-class angles — extraction within 1 ulp, trig
    matches the correctly-rounded reference) and the subnormal square
    `|B²| = 2^-2148` → angle `2^-1074` (survives exponent composition; direct
    f64 conversion of the square is non-conforming).

**Inverse acceptance (both orders, sound denominators):** for `X = inverse(M)`,
require `‖M·X − 1‖_coeff ≤ 1e-6 · max(‖M‖_coeff · ‖X‖_coeff, 1.0)` AND
`‖X·M − 1‖_coeff ≤ 1e-6 · max(‖M‖_coeff · ‖X‖_coeff, 1.0)` (backward-error-style
denominator — a candidate like `X = 2e8` for `M = 1e-8`, which passed the earlier
unsound `‖MX−1‖/‖X‖` form despite 100% forward error, now fails); plus
forward-accuracy agreement with the independent reference where conditioning
permits (κ documented per case). Independent references throughout: exact-rational
oracle on small signatures; independently written Gaussian elimination for the
inverse; high-precision series (not the production series) for exp.

Plus D07 baseline gates and catalog regeneration from the split layout (CORE-CAT1:
regenerate from the merged tree post-merge). Staged dispatches with verification
between stages; review loop to 0 P1/P2 (max 6 rounds); user merges.

## Out of scope

Rotor/log domain policy beyond the named error-propagation ripple (W07 / D03–D05),
Hodge dual (CORE-08), slerp/reflection (CORE-09–15), other grade-membership sites
outside inverse/exp, `amari-gpu`/`amari-wasm` implementation audits (obligations
only), `normalize_versor` publicity.
