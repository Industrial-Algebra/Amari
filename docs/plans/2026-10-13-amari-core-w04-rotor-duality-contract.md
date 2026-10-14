# CORE-W04 contract — rotor domains, duality, and grade-type enforcement (D03/D04)

Status: REVISION 9 — review loop converged (R1 5/9 → R2 2/4/2 → R3 1/3 →
R4 2/1 → R5 2/2 → R6 1/2/2 → R7 1/1/1 → R8 **0 P1 / 0 P2 / 2 P3**; both
P3 documentation corrections applied). Awaiting operator approval. Operator decisions D03.1–D03.3 and D04 recorded 2026-10-13 via
ask_user. Base: develop `79a9653`. Covers **CORE-08 through CORE-15**.

## 0. Scope and closed findings

| Finding | Class | Resolution vehicle |
|---|---|---|
| CORE-08 Hodge contradiction | P2 | D04 dual split (`formal_complement` + metric `hodge_dual`) |
| CORE-09 wrongly graded wrappers | P1 | D03.1 private representation + D04 typed duals |
| CORE-10 slerp hard-codes 8 | P1 | D03.2 log-based interpolation |
| CORE-11 log/power laws | P1 | D03.2 domain-aware logarithm + exact integer powers |
| CORE-12 from_vectors metric assumption | P1 | D03.3 metric-aware construction |
| CORE-13 reflection convention mismatch | P1 | D03.3 `reflect` (hyperplane, breaking) + `reflect_line` |
| CORE-14 unchecked rotor construction | P2 | D03.1 checked constructors (grade validation) |
| CORE-15 3D-only component layouts | P2 | D04 dimension-general component APIs |

Excluded: `VerifiedRotor` cohort; Cayley caching (PERF-01); multi-plane rotor
logarithms beyond certified classes.

## 1. D03.1 — private representation and checked construction

1. `Scalar`, `Vector`, `Bivector` fields become private. Public escapes:
   `as_multivector(&self) -> &Multivector`, `into_multivector(self)`, and
   `TryFrom<Multivector>` returning `Err(GradeError)` when any foreign-grade
   coefficient is nonzero. Exact predicate `coefficient != 0.0`: `±0.0` is
   zero; any nonzero foreign-grade coefficient — including NaN or infinity —
   rejects. NaN/infinity in allowed grades is documented out of scope for
   wrapper conversion; the checked constructors of §2–§4 that require
   finiteness say so.
2. Public constructors preserved; `from_multivector` becomes
   `try_from_multivector` + `TryFrom`. Infallible constructors stay
   grade-correct by construction.
3. `Rotor::from_multivector_bivector -> Result<Self, RotorError>` (R2-P2-5:
   now in the §5 table) validates grade-2-ness exactly →
   `Err(NotABivector)`; all normalization fallbacks are deleted — the four
   current sites (`rotor.rs` `from_bivector`, `from_multivector_bivector`,
   `inverse`-adjacent, `power`) become explicit errors:
   `Err(NotNormalizable)` for normalization failure. Silent identity
   fallbacks are gone everywhere.
4. **Rotor value-space invariant (construction closure):** the value space is
   the closure of the constructors — all of which produce versors with even
   grades and **positive reverse norm `RR† > 0`** (identity component of the
   Spin group). Constructors normalize (stored `RR† = 1 ± ulp`); `compose`
   (§1.5) is unnormalized, so chained compositions drift from unit —
   documented. Evenness alone does not imply versor behavior (Cl(6,0,0)
   `(1+I)/√2` counterexample); the guarantee is the closure, and
   `apply_to_vector`'s grade-0+1 projection is documented defense-in-depth,
   not the invariant (one `debug_assert` on even grades retained).
   R2-verified: no public `TryFrom<Multivector> for Rotor`, mutable accessor,
   or arbitrary-even conversion path exists after §1.1/§1.3 — the raw
   constructor escape is closed.
4b. **Stored-element inverse and action semantics (R3-P1, revised for
   R4-P1-1):** element logs preserve the stored scalar scale and `compose`
   is unnormalized, so `inverse()` must be the inverse of the **stored
   element** — and finite composition can destroy invertibility **without
   overflow** (R4: the exactly-unit boost `5/4 + (3/4)e12` squared to
   `R^32 = [2^31, 2^31]` has certified stored `N = w² − β² = 0` — all
   coefficients finite; the mathematical norm is 1, but the f64 pair can
   no longer certify it). Therefore:
   - `inverse() -> Result<Self, RotorError>` (breaking), **uniformly
     `R† / N`** with `N = RR†` the certified **scalar** reverse norm
     (R5-P2-1: the `exp(−log)` route is dominated — near the hyperbolic
     boundary `w = 1, β = nextafter(1,0)` it returns
     `[4503599627370493.0, −4503599627370492.5]` while `R†/N` returns the
     accurate `[4503599627370496.0, −4503599627370495.5]`; the exact-`N`
     division gives ≤1 rounding per coefficient unconditionally, so the
     log route is deleted). Requirements: `N` must certify **scalar and >
     0**; `N ≤ 0` or non-scalar `N` → `Err(Degenerate)`. **Scope (R6-P2-2):
     this gate certifies reverse-norm/invertibility, not versor
     membership** — Cl(6,0,0) `R = 1+I` has `RR† = 2` scalar positive yet
     `R e1 R†/2 = −e1·I` is grade-5. Versor behavior is guaranteed only by
     the §1.4 construction closure (with the floating-storage caveat);
     `apply`'s grade-0+1 projection remains the documented defense for
     storage drift, not a certificate. Works for negative-sheet elements (their `N > 0`;
     log rejects them but inverse does not depend on log). "Single
     rounding" claims are scoped to exponential stages only, never to
     end-to-end log→exp chains (R5-P2-1 wording rule).
   - `apply`/`apply_to_vector -> Result<_, RotorError>` (breaking; the
     R^32-stored value would silently produce a zero/garbage sandwich
     today): `R · A · R† / N` with the same certified scalar-`N` gate;
     `N ≤ 0`/non-scalar → `Err(Degenerate)`. Exactly-normalized rotors
     behave as today within rounding.
   - `compose` (§1.5) stays raw and infallible and is thereby the
     **documented domain-exit point**: a stored result may carry `N ≤ 0`;
     every checked operation detects it. Large-rapidity boost chains are a
     finite-representation limit (disclosed §8): parameterize by
     generator+angle (`from_bivector`) rather than composing stored
     coefficients.
   Baseline defect recorded: today `inverse()` returns the bare reverse
   (boost fixture observed `[1.127625965206381, 0, 0, 0.5210953054937475]`;
   stored-element inverse rounds to `[1.1276259652063807, 0, 0,
   0.5210953054937474]`; exact `N = 20282409603651673759639097286673 /
   20282409603651670423947251286016 ≠ 1`). The `power_integer(−1)`
   bit-exact pin refers to this newly specified inverse and must not
   freeze today's reverse-only implementation.
5. `compose` stays **infallible**: it is the raw geometric product of two
   in-domain rotors (versor × versor = versor; no internal normalization, no
   fallback). Non-finite overflow inside compose is documented as producing
   non-finite coefficients (finite-domain callers use §2/§3 checked paths).
6. Migration surface (verified 2026-10-13): `amari-automata/self_assembly.rs`
   ×2, `amari-discovery/probes/network.rs` ×1; internal `mv:` literals (133)
   migrate mechanically. Re-verified at stage gates.

## 2. D03.2 — domain-aware logarithm, power, interpolation

`Rotor::logarithm() -> Result<Multivector, RotorError>` (breaking). No
floating-point threshold participates in routing; classification reuses
W03's exact dyadic square certification (`exact_x2`). The log is the
**element log of the stored value** (R2-P1-1), including the scalar scale.

Let `R = w + B + H`, `B` grade-2, `H` = all grade-≥4 content.

**a. Scalar-only first:** `B = 0`, `H = 0` exactly:
- `w = 1` → `Ok(zero)` exactly (pin).
- `w > 0`, `w ≠ 1` → `Ok(ln w)` (disclosed: `exp(ln w) = w` within rounding,
  not bit-exact — e.g. `exp(ln 3)` can return `3.0000000000000004`).
- `w = 0` (including signed zero; reachable via compose domain exits:
  `Q₊·Q₋ = [2³¹,2³¹]·[2³¹,−2³¹] = 0` exactly — R5-P2-2) →
  `Err(NotALogDomain)` — battery pin.
- `w < 0` → `Err(BranchAmbiguous)` — a π plane a scalar cannot name.
- Non-finite `w` → `Err(NotALogDomain)`.

**b. `H ≠ 0` (any nonzero grade-≥4 content):** `Err(Uncertified)`
(multi-plane). Exact single-plane exponentials carry only scalar+bivector
parts, so no certified class is excluded; stored floating rotors can lose
exact simplicity to rounding — disclosed caveat.

**c. `H = 0, B ≠ 0`:** classify `B²` exactly (`β = sqrt(∓B²)` from the
certified value; finiteness of `w` required throughout):
- **Elliptic** `B² = −β² < 0`:
  `log R = ½·ln(w² + β²) + atan2(β, w)·(B/β)`
  — the scalar scale makes this the element log:
  `exp` of it returns `w + B` exactly in exact arithmetic
  (`√(w²+β²)·(w + B)/√(w²+β²)`). Principal branch `atan2 ∈ (−π, π]`.
  For exactly-unit stored rotors the scale is `½ln(1) = 0`; approximately
  unit stored values give scalar parts at the ~1e−16 scale (honest element
  log, disclosed). `β > 0` whenever `B ≠ 0` (R2-verified), so the
  `atan2(−0.0, w)` edge is not admissible here.
- **Hyperbolic** `B² = +β² > 0`: domain = positive principal sheet —
  **accept iff `w > 0` (finite, stored) and certified `N = w² − β² > 0`
  — these two alone suffice** (R7-verified: `w > 0 ∧ N > 0 ⟹ w > |β|`;
  the revision-7 extra clause `w + β > w` is deleted — R7-P1: it rejects
  the valid small boost `R = 1 + 2⁻⁵⁴e12`, whose certified
  `N = (2¹⁰⁸−1)/2¹⁰⁸ > 0` but whose rounded `w + β = w`). Negative-sheet
  elements (`w = −5/4 + …`) reject via `w > 0`; `N = 0` is noninvertible
  and rejects.
  **Evaluation (R6-P1 + R7-P1 + R7-P2, unified and range-safe; R8-P3-1
  terminology):** with rounded `β = √(B²_certified)` and certified `N`,
  form the **exact rational `q = (w+β)² / N` represented by its dyadic
  numerator and denominator kept separately** (no f64 materialization of
  the square or quotient; `q` itself is generally not dyadic), and:
  - `δ_num := (w+β)² − N` (exact dyadic); branch comparison
    `|δ_num| < N`; then `δ = δ_num / N` evaluated as one correctly
    rounded quotient (or exactly, when `N` is a power of two): if
    `|δ| < 1`, **`φ = ½·ln1p(δ)`** — cancellation-safe small rapidity:
    for `R = 1 + 2⁻⁵⁴e12`, exact `δ = 2/18014398509481983` (which rounds
    to `2⁻⁵³`), `φ = 2⁻⁵⁴ = 5.551115123125783e−17`, the correctly
    rounded rapidity; for the R6 mixed-component fixture
    `R = 2²⁷ + 2²⁷e13 + e12` with `N = 1`, exact `δ = 2⁵⁶ − 1` (displayed
    values rounded), `φ = ½·ln(2⁵⁶) ≈ 28·ln2 ≈ 19.40812105567846865` —
    finite and correct, and `B/β = e13 + 2⁻²⁷e12` exactly representable
    with exact square `1 − 2⁻⁵⁴` (not 1 — disclosed; `K` is unit to
    rounding));
  - else `φ = ½·ln q` with `ln q = ln m + e·ln 2` for the rational's
    mantissa/exponent decomposition (`m ∈ [1,2)` — range-safe by
    construction: no overflowing norm or sum is ever materialized).
  Scalar scale `½·ln N` evaluated by the same mantissa/exponent
  decomposition (absorbs R2's ~1.6e−16 stored-norm deviation;
  disclosed). **General requirement (R7-P2): every intermediate scalar
  in log evaluation — elliptic scale `½·ln(w²+β²)` included — is
  computed on exact dyadic rationals with symbolic mantissa/exponent
  decomposition (W03 machinery); direct f64 evaluation of `w²+β²` (which
  returns inf for `w = β = 1e154` though the true scale is
  `354.944677911363…`) is forbidden.** Finite-result/error behavior:
  overflow of the final coefficients follows the output-`N` gate;
  evaluation-stage non-finiteness → `Err(NotALogDomain)`. Batteries: the
  small-boost fixture (accepted, rapidity `5.551115123125783e−17`); the
  mixed-component fixture (accepted, `28·ln2`); elliptic `w = β = 1e154`
  (finite scale `354.94…`); hyperbolic `w = 1e308, β = nextafter(w,0)`
  (finite rapidity `18.42172174950956…`).
- **Parabolic** `B² = 0, B ≠ 0`: require `w > 0`; `log R = ln w + B/w`
  exactly (`(B/w)² = 0` ⟹ round-trip `w + B` in exact arithmetic);
  `w ≤ 0` → `Err(NotALogDomain)`.
- `B²` non-scalar → `Err(Uncertified)`.

**Round-trip scoping (R2-P2-4, R6-P2-1, R7-P3):** the batteries assert the
two directions separately with **conditioning-/fixture-dependent end-to-end
budgets — no single-rounding claim applies to the composite chain**. Witness
(R8-verified, `w = 1`, `b = nextafter(1,0)`, correctly rounded scalar
logarithms — scale `−18.021826694558577`, rapidity `18.714973875118524` —
then high-precision exponential rounded to f64): returns
`[1.000000000000002, 1.0000000000000018]` — 9 and 17 ulp; the
conditioning warning stands and that fixture's budget is set accordingly
(`[0.9999999999999983, 0.9999999999999982]` is reproducible only with a
one-ulp-lower rapidity and is not the stated-route result). `log(exp X) ≈ X`
**only for `X` in the selected principal-branch domain** (`exp(2πe12) = 1`
and `log(1) = 0` is branch selection, not an error).

**d. Powers (R2-P2-3):**
- `power_integer(n: i64) -> Result<Self, RotorError>`: `n = 0` → identity;
  `n = 1` → `self` clone (**bit-exact**); `n = −1` → `self.inverse()`
  (direct return of the §1.4b route; **bit-exact relative to that inverse**);
  other `n`: **certified log class → `exp(n · log R)`** (integer-scaled
  log, W03 pipeline — no composition round-off chain; normative route per
  R4-P2); uncertified multi-plane rotors → binary exponentiation with
  `compose` (addition chains permitted only if the implementation states
  the chain per exponent; `N ≤ 0` detection at each checked boundary →
  `Err(Degenerate)`). **All checked rotor-producing operations —
  `power`, `power_integer`, `from_bivector`, `from_axis_angle`,
  `from_vectors`, `slerp` — certify output scalar `N > 0`** (R5-P1-2: the
  closed form itself cannot preserve positive stored `N` at magnitude —
  even the exact `cosh(32·ln2) = 2³¹ + 2⁻³³` rounds to `2³¹`, storing
  `N = 0`; such outputs return `Err(Degenerate)`: generator+angle
  parameterization avoids composition error but not coefficient
  **representability**). "Log-class rotors never compose" is a description
  of the integer-power evaluation route only; arbitrary callers may
  compose (domain-exit documented). Composition
  laws `power_integer(a)∘power_integer(b) ≈ power_integer(a+b)` are asserted
  with the **fixture-scoped error budget**: `≤ (m + 1) ulps` on the
  max-abs coefficient norm, where `m` = the number of compositions the
  implementation used for that exponent (for 7: `m ≥ 4`, not `⌈log₂7⌉`);
  a multivector composition is not a single correctly-rounded operation.
  Bit-exact composition assertions are restricted to fixtures whose
  coefficients make every intermediate product exact (exactly-representable
  dyadic constructions; **not** binary-fraction angles, whose trig
  coefficients are not generally exact — R2 finding).
- `power(t: f64) -> Result<Self, RotorError>`: `t` exactly integral in i64
  range → `power_integer`; else `exp(t · log R)` through the W03 pipeline.
  Errors: `RotorError::Exp(ExpError)` carrier (lossless `From`); log
  failures propagate directly; composition overflow →
  `RotorError::Composition`.

**e. slerp:** `slerp(&self, other, t) -> Result<Self, RotorError>`:
- `t = 0` → `self` exactly; `t = 1` → the **selected representative**
  (`other` or `−other`, sign rule below) — direct returns, bit-exact.
- Sign rule: the **scalar coefficient dot** `⟨A,B⟩_dot := Σᵢ AᵢBᵢ` over all
  coefficients (basis-orthonormal dot — the quaternion convention; **not**
  `Multivector::inner_product`, which returns a multivector: for
  `A = 0.6+0.8e12`, `B = 0.28+0.96e12` the scalar part of the inner product
  is `−0.6` while the coefficient dot is `+0.936` — R3 fixture). If
  `⟨A,B⟩_dot < 0`, the selected representative is `B* = −B`, and the
  interior formula and the `t = 1` endpoint both use `B*` ("sign-selected
  representative"; no shortest-geodesic claim for indefinite groups). Red
  battery: this differing-sign fixture at an interior `t`, plus a
  negative-coefficient-dot fixture.
- `t ∈ (0,1)`: `A·(A⁻¹B)^t` via `power`; certified log required; `Err`
  propagates. `A = B` → `A` exactly. Hard-coded 8-coefficient loop deleted.
  Pin: `identity.slerp(r, 1.0)` returns `r` exactly for a certified
  single-plane Cl(4,0,0) rotor.

## 3. D03.3 — from_vectors and reflection

**a. `from_vectors(a, b) -> Result<Rotor, RotorError>`** (breaking: was
`Option`). Exact pure-vector validation on both inputs (the §1.1 predicate
on all non-grade-1 coefficients) → `Err(NotAVector)` for foreign-grade
inputs, checked **before** square/candidate processing (routing precedence:
NotAVector → NullVector → CausalTypeMismatch → antipodal/trichotomy).
`from_vectors_mv(scalar(1), e1)` → `Err(NotAVector)` — red pin.
`reflect`'s normal check follows the same precedence: `NotAVector` before
`NotUnitNormal`. `σ(x) = x·x` certified:

1. `σ = 0` either input → `Err(NullVector)`.
2. Equal causal type: `sgn σ(a) = sgn σ(b) =: s ∈ {±1}` (the sandwich
   preserves the square) → else `Err(CausalTypeMismatch)`.
3. Directions `â, b̂` normalized (finite-guarded). **Candidate
   `C = 1 + s·b̂â`** (then `Câ = b̂C` and `C = s·(b̂+â)·â` exactly — a
   two-vector product, hence a versor; both identities asserted in the red
   battery for both signs of `s`).
4. **Direction pre-test on certified ORIGINAL inputs (R4-P1-2 + R5-P1-1 —
   rounded normalization cannot be trusted for direction equality, and
   Cauchy–Schwarz equality is invalid in indefinite signatures: in
   Cl(0,2,0) `a=b=e1` gives `⟨a,b⟩ = −1 < 0` with Gram equality — falsely
   "antipodal" — while `a=e1, b=−e1` gives `⟨a,b⟩ = +1` — falsely
   "same-direction"; in Cl(2,1,0) `a=e1, b=−e1+e2+e3` gives Gram equality
   without linear dependence at all):** certify **exact proportionality**
   of the original coefficient vectors, signature-free: all 2×2 minors
   `aᵢ·bⱼ − aⱼ·bᵢ = 0` exactly (dyadic products — no sqrt, no metric).
   If proportional: the factor's sign comes from `sign(aᵢ·bᵢ)` at any
   index with `bᵢ ≠ 0` — negative → **antipodal** (Euclidean 2D resolves
   via the unique plane — both `±e12` implement the same action, no sign
   pin — battery: the `e1+e2 ↦ −(e1+e2)` fixture maps `â` to `−â`;
   otherwise `Err(Antipodal)`); positive → same-direction (the candidate
   trichotomy handles it: `C` scalar, `CC† > 0`, identity action — Cl(0,2,0)
   `a=b=e1` fixture). Not proportional → proceed to the trichotomy (the
   Cl(2,1,0) non-parallel case reaches `NullCandidate` as R2 required).
   Near-antipodal non-proportional inputs proceed with a disclosed
   conditioning caveat (action accuracy degrades as the gap to π
   shrinks).
5. **Candidate trichotomy (R2-P1-2 — positivity is certified
   independently, never assumed):**
   - `C = 0` exactly (the rounded-candidate degenerate case; unreachable
     after the pre-test but retained as an exact test): antipodal — as
     above.
   - `C ≠ 0`: compute `CC†` exactly (certified scalar;
     `CC† = 2(1 + s·⟨â,b̂⟩)` analytically for exactly normalized inputs —
     **no universal upper bound exists** (R3-P2: Cl(1,1,0)
     `b̂ = (5/4)e1+(3/4)e2` gives `CC† = 9/2`; positive-sheet boosts are
     unbounded above); Cl(1,1,0) `b̂ = −(5/4)e1 + (3/4)e2` gives
     `CC† = −1/2` and Cl(2,1,0) `b̂ = −e1+e2+e3` gives `CC† = 0`, both
     R2-reproduced):
     - `CC† > 0` → normalize (finite-guarded) → `Rotor`.
     - `CC† = 0` → zero-divisor candidate (distinct from antipodal;
       R2-reproduced in Cl(2,1,0)) → `Err(NullCandidate)`.
     - `CC† < 0` → the connecting even versor lies off the identity
       component (sheet change — e.g. boosts past the horizon) →
       `Err(SheetChange)`.
6. **Action pin:** `R·â·R⁻¹ = b̂` within the disclosed rounding convention —
   Cl(0,2,0) `e1 → e2` (today: spurious `None`), `e1 → e1` → identity
   (today: `None`), and the non-axis antipodal fixture of item 4.

**b. `reflect` — hyperplane, docs win (breaking):**
`reflect(n, A) = n · ĉ(A) · n⁻¹`, `ĉ` = grade involution (grade-k ×
`(−1)^k`). Outermorphism of `v ↦ −nvn⁻¹`: scalars fixed, perpendicular
blades preserved, vectors negated along `n`. Certified pure-vector `n` with
`n·n = ±1` required, else `Err(NotUnitNormal)`. Pins: `reflect(e1,e1) =
−e1`; `reflect(e1,1) = 1`; `reflect(e1,e23) = e23`; involution
`reflect(n, reflect(n,A)) = A` (disclosed rounding; battery).

**c. `reflect_line(n, A) = n·A·n⁻¹`** — versor sandwich (correct
outermorphism of the line map; no involution). The migrated existing test
asserts `reflect_line(e1,e1) = +e1` verbatim.

## 4. D04 — dual operations and dimension-general components

1. `formal_complement()` — permutation-sign-only, all signatures:
   `e_A ↦ sign·e_{A^c}`, sign = inversion parity between sorted `A` and
   sorted `A^c`; `formal_complement²` on grade `k` is `(−1)^{k(n−k)}`
   (documented). Pin: `formal_complement(e3) = e12` in Cl(2,0,1).
2. `hodge_dual() -> Result<Self, DualError>` — metric operation,
   **`A ↦ reverse(A)·I`**, `I` the certified pseudoscalar, requiring `I`
   invertible; **degenerate rejection is `R > 0`** (nondegenerate `R = 0`
   is fine). Euclidean 3D table preserved: `⋆e1 = e23`, `⋆e2 = −e13`,
   `⋆e3 = e12`, `⋆e12 = e3` — consistent double-dual (R2-verified). The
   grade-dependent reversal sign is the mechanism that yields the standard
   table; **multiplication-side differences are immaterial in 3D** (`I` is
   central there) and are discussed only for dimensions where sides differ
   (R2-P3-7: the draft's earlier side-explanation was false and is
   withdrawn).
3. Typed duals, receiver-signature-preserving concrete impls:
   `impl Vector<3,0,0>` → `Bivector<3,0,0>` and `impl Vector<0,3,0>` →
   `Bivector<0,3,0>` (hodge_dual Result, formal_complement infallible);
   corresponding `Bivector` duals return the matching `Vector`. The generic
   `Vector::<P,Q,R>::hodge_dual() -> Bivector` is deleted (CORE-09 specimen
   becomes a compile error). Other dimensions/signatures: `Multivector`
   methods.
4. Dimension-general components (CORE-15): zero-based lexicographic order
   (4D: `0↦e12, 1↦e13, 2↦e14, 3↦e23, 4↦e24, 5↦e34`).
   `Bivector::component(k) -> Option<f64>`; `set_component(&mut self, k, v)
   -> bool` (false = out of range, no write); `Multivector::
   set_bivector_component(&mut self, k, v) -> bool` (same contract; the
   current silent-zero behavior is a bug — red pin: Cl(4,0,0) component 3
   round-trips e23). `Bivector::zero()`; `Bivector::e_ij(i, j) ->
   Result<Self, ComponentError>` (dedicated error — R2-P3-8; strict
   `1 ≤ i < j ≤ n`, reversed indices rejected with the sign rule
   documented). The `[3,5,6]` layout survives only inside 3D-concrete
   impls delegating to the general map. Existing blade-index `Index`/`get`
   keep blade semantics; the two indexing systems documented side by side.
   `to_rotation_matrix`/axis-angle helpers move to 3D-concrete impls.

## 5. Error surface (normative signature table)

```rust
RotorError { NotAVector, NotABivector, NotNormalizable, NotALogDomain,
             BranchAmbiguous, Uncertified, NullVector, Antipodal,
             CausalTypeMismatch, NullCandidate, SheetChange, NotUnitNormal,
             NotEven, Degenerate, Composition, Exp(ExpError) } // From<ExpError>
DualError { DegeneratePseudoscalar }
GradeError { ForeignGrade }
ComponentError { OutOfRange }

fn logarithm(&self)                      -> Result<Multivector, RotorError>
fn power(&self, t: f64)                  -> Result<Self, RotorError>
fn power_integer(&self, n: i64)          -> Result<Self, RotorError>
fn slerp(&self, o: &Self, t: f64)        -> Result<Self, RotorError>
fn from_vectors(a: &Vector, b: &Vector)  -> Result<Self, RotorError>
fn from_vectors_mv(a: &Multivector, b: &Multivector) -> Result<Self, RotorError>
fn from_bivector(&Bivector, f64)         -> Result<Self, RotorError>   // Exp-carried exp stage
fn from_axis_angle(&Vector, f64)         -> Result<Self, RotorError>
fn from_multivector_bivector(&Multivector, f64) -> Result<Self, RotorError>
fn inverse(&self)                       -> Result<Self, RotorError>     // §1.4b uniform R†/N
fn apply(&self, A: &Multivector)         -> Result<Multivector, RotorError>
fn apply_to_vector(&self, v: &Vector)    -> Result<Vector, RotorError>
fn reflect(n: &Multivector, A: &Multivector) -> Result<Multivector, RotorError>
fn reflect_line(n: &Multivector, A: &Multivector) -> Result<Multivector, RotorError>
fn compose(&self, o: &Self)              -> Self                        // §1.5 raw; domain-exit point
fn to_rotation_matrix(&self)             -> Result<[[f64; 3]; 3], RotorError> // Rotor<3,0,0>; added R4-F18/R5-F21
fn hodge_dual(&self)                     -> Result<Self, DualError>      // Multivector
fn formal_complement(&self)              -> Self                          // Multivector
fn e_ij(i: usize, j: usize)              -> Result<Self, ComponentError> // Bivector
fn component(&self, k: usize)            -> Option<f64>                   // Bivector
fn set_component(&mut self, k: usize, v: f64) -> bool                    // Bivector
fn set_bivector_component(&mut self, k: usize, v: f64) -> bool           // Multivector
// Named basis-blade constructors made fallible by R5-F20 (dimensional domain):
fn e12()                                 -> Result<Self, ComponentError>  // Bivector; n >= 2; added R5-F20
fn e13()                                 -> Result<Self, ComponentError>  // Bivector; n >= 3; added R5-F20
fn e23()                                 -> Result<Self, ComponentError>  // Bivector; n >= 3; added R5-F20
fn e1()/e2()/e3()                        -> Result<Self, ComponentError>  // Vector; n >= 1/2/3; added R5-F20
fn e1()/e2()/e3()/e12()/e23()/e31()/e123()
                                        -> Result<Multivector<P,Q,R>, ComponentError> // Basis; n per blade; added R5-F20
// §1.1 escape/conversion surface (listed here for migration completeness):
fn as_multivector(&self)                 -> &Multivector                  // Scalar/Vector/Bivector
fn into_multivector(self)                -> Multivector                   // Scalar/Vector/Bivector
impl TryFrom<Multivector> for Scalar/Vector/Bivector                       // Err(GradeError)
```

`from_bivector`/`from_axis_angle` migrate to `Result<_, RotorError>` (R2-P2-5:
`ExpError` has no normalization carrier; the exp stage arrives via the
`Exp(ExpError)` conversion). `Option` returns are eliminated from the rotor
construction surface.

## 6. Red-first inventory (verified 2026-10-13 on `79a9653`; baseline vs target separated per R2-P2-6)

1. CORE-08: baseline `star(e3)` = zero in Cl(2,0,1); target
   `formal_complement(e3) = e12`, `hodge_dual` → `Err(DegeneratePseudoscalar)`.
2. CORE-09: baseline `Vector::<4,0,0>::e1().hodge_dual()` wrongly-typed Ok;
   target: compile error; `mv`-forge → compile error or `TryFrom` Err.
3. CORE-10: baseline slerp panic Cl(2,0,0) / silent identity Cl(4,0,0);
   target `Ok(identity)` / exact `r`.
4. CORE-11 (fixture = unit `e12` boost, angle 1): baseline `power(1)`
   returns scalar **1**, bivector **0**; stored rotor is scalar
   **1.127625965206381**, bivector **−0.5210953054937475**; target:
   `power_integer(1)` returns the **stored coefficients bit-exact** (direct
   clone). Small-angle `1e-8`: baseline drops the bivector; target preserves
   it (atan2 route). `2π` rotor: baseline returns +1; target
   `power_integer(1)` clones the stored −1-scalar rotor **bit-exact**
   (including its ~1e−16 residual bivector — the pin is the clone, not
   mathematical zero).
5. CORE-12: Cl(0,2,0) baseline spurious `None` both cases; target action
   `ê1 ↦ ê2` (disclosed rounding) and identity.
6. CORE-13: baseline `reflect(e1,e1) = +e1`; target −e1 (plus scalar and
   e23 pins); `reflect_line(e1,e1) = +e1` (migrated test).
7. CORE-14: baseline `Ok` with grade-1 coeff; target `Err(NotABivector)`.
8. CORE-15: baseline silent zero; target Cl(4,0,0) component-3 e23
   round-trip, `Bivector::<4,0,0>::zero()` exists, out-of-range false,
   `e_ij(2,1)` → `Err(OutOfRange)`.
9. Inverse/action baseline defects (R3-P1; oracle labeling per R4-P2):
   boost fixture `inverse()` observed `[1.127625965206381, 0, 0,
   0.5210953054937475]`; target = stored-element inverse
   `[1.1276259652063807, 0, 0, 0.5210953054937474]` (§1.4b `R†/N`, ≤1
   rounding per coefficient). `power_integer(−2)`: **exact-arithmetic
   oracle** 1.5430806348152437 within ≤2 ulp of the normative output
   (observed raw compose 1.5430806348152435).
9b. R4/R5 finite-closure battery: the dyadic boost chain `5/4+(3/4)e12`
   — `R^16 = [32768.000007629395, 32767.999992370605]` (stored `N = 1`);
   `R^32 = [2^31, 2^31]` certified stored `N = 0`: **both** routes return
   `Err(Degenerate)` — `inverse()`/`apply` directly, and
   `power_integer(32)` via the closed form (R5-P1-2: even the exact
   exponential rounds to `N = 0` at this magnitude — representability
   limit, disclosed §8). Compose-chain users likewise.
9c. Direction pre-test battery (R5-P1-1 cases): Cl(0,2,0) `a=b=e1` →
   identity rotor (not "antipodal"); `a=e1, b=−e1` → antipodal; Cl(2,1,0)
   `a=e1, b=−e1+e2+e3` → not proportional → `NullCandidate` (preserved);
   Euclidean 2D non-axis `a=e1+e2, b=−a` → resolved, maps `â` to `−â`.
9d. Zero-rotor log pin: `Q₊·Q₋ = 0` stored zero rotor → `logarithm()` →
   `Err(NotALogDomain)` (R5-P2-2).
9e. Hyperbolic evaluation batteries: `R = 2²⁷ + 2²⁷e13 + e12` in
   Cl(2,1,0) (constructible by exponentiating `e13 + 2⁻²⁷e12` and
   normalizing) → accepted, rapidity `28·ln2 ≈ 19.40812105567846865`;
   small boost `R = 1 + 2⁻⁵⁴e12` → accepted (gate `w>0 ∧ N>0` alone),
   rapidity `5.551115123125783e−17`; range extremes: elliptic
   `w = β = 1e154` → finite scale `354.944677911363…`; hyperbolic
   `w = 1e308, β = nextafter(w,0)` → finite rapidity `18.42172174950956`;
   no `β/w` ratio or overflowing norm formed anywhere; round-trips within
   the end-to-end budgets.
9f. Versor-gate scope documentation test: `apply` doc + a unit test
   asserting the Cl(6,0,0) `1+I` sandwich (via raw multivector arithmetic)
   is grade-5 — pinning the "not a versor certificate" scope statement.
10. from_vectors/slerp routing: `from_vectors_mv(scalar(1), e1)` →
   `Err(NotAVector)`; slerp differing-sign fixture (coefficient dot +0.936
   selects `B`, inner-product-scalar `−0.6` would select `−B`) at interior
   `t` and negative-coefficient-dot fixture.
11. Law batteries (scoped per §2d/§2e): `power_integer` {0,1,−1} bit-exact;
   composition per the fixture-scoped `m+1`-ulp budget; slerp endpoints
   exact (selected representative); `exp(log R) ≈ R` for accepted elements
   (disclosed convention); `log(exp X) ≈ X` only in the principal domain;
   reflect involution; `formal_complement²` sign law; scalar branch
   `log(1) = 0` exactly; negative-sheet `−5/4+(3/4)e12` →
   `Err(NotALogDomain)`; Cl(6,0,0) `I`-rotor log → `Err(Uncertified)`;
   from_vectors trichotomy reproductions (Cl(1,1,0) `CC†<0` → SheetChange;
   Cl(2,1,0) `CC†=0` → NullCandidate) now green as named errors.

## 7. Stages

1. **S1 errors + privacy**: error enums + §5 signature table; `mv` private +
   escapes + TryFrom; fallback deletion; **§1.4b `inverse`/`apply`/
   `apply_to_vector` Result migration with the certified scalar-`N` gate and
   its red batteries (R^32 chain, zero rotor, negative-sheet inverse)**;
   **constructor output-`N` gates on `from_bivector`/`from_axis_angle`/
   `from_vectors`** (checked before successful returns, so endpoint/clone
   pins are bit-exact on success); consumer migration; workspace green.
2. **S2 logarithm/powers**: §2 a–d (including the R6 hyperbolic
   certified-`N` evaluation and mixed-component fixture); law batteries.
3. **S3 slerp + from_vectors + reflect**: §2e, §3.
4. **S4 duals + components**: §4.
5. **S5 both-compiler matrix + catalog + evidence + SUMMARY** (D07 recipe;
   W03 evidence-hygiene scrub line applied from the start).

## 8. Disclosures

- Multi-plane (nonzero grade-≥4 content) rotor logs: uncertified → `Err`.
- Negative scalars (including `−1`): `Err(BranchAmbiguous)`.
- Hyperbolic logs restricted to the positive principal sheet (`w > β ≥ 0`).
- `inverse()`/`apply` operate on the **stored element** via the §1.4b
  routes; exactly-normalized rotors behave as before, unnormalized chains
  correct today's stored-scale error.
- **Finite-representation limit:** raw `compose` chains of large-rapidity
  boosts can store `N ≤ 0` without overflow (R4 `R^32` chain), and the
  closed-form exponential itself rounds to `N = 0` at the same magnitude
  (R5) — generator+angle parameterization avoids composition error but
  **not coefficient representability**; all checked rotor-producing
  operations certify output scalar `N > 0` and return `Err(Degenerate)`
  otherwise. Near-antipodal `from_vectors` inputs carry a conditioning
  caveat. End-to-end log→exp accuracy is conditioning-bounded; only
  exponential stages carry single-rounding claims.
- Element-log scalar scales and rapidities are evaluated on exact dyadic
  rationals with symbolic mantissa/exponent decomposition (hyperbolic via
  `q = (w+β)²/N`, small-`δ` `ln1p` route): approximately-unit
  stored rotors give ~1e−16 scalar parts — the honest log of the stored
  value; `exp(ln w)` round-trips are rounding-bounded, not bit-exact;
  **end-to-end `exp(log R)` accuracy is conditioning-bounded per fixture
  (near-boundary cases reach 9–17 ulp) — no composite single-rounding
  claim**.
- `log(exp X) = X` only within the principal-branch domain.
- from_vectors candidate trichotomy: `CC† ≤ 0` candidates exist for equal
  causal types and non-antipodal inputs (indefinite signatures); they are
  rejected as `NullCandidate`/`SheetChange`, not normalized.
- Antipodal resolution: exact Euclidean 2D only; otherwise `Err`.
- `power_integer` composition: fixture-scoped `m+1`-ulp budget; bit-exact
  only {0,1,−1} and exact-product fixtures.
- slerp endpoints return the selected representative; no shortest-path
  claim for indefinite groups.
- Metric `hodge_dual`: `reverse(A)·I`; degenerate `R > 0` → `Err`; side
  differences immaterial in 3D, relevant only in dimensions where `I` is
  non-central.
- Typed duals only on the concrete 3D signatures of §4.3.
- Rounding can strip exact simplicity from stored rotors (certification
  caveat).
- Randomized reviewer batteries are evidence, not proof.
- Breaking blast radius: `.mv` readers (3 external sites), `reflect`
  semantics, infallible log/slerp/from_vectors/hodge signatures,
  component-index layout, `from_bivector`/`from_axis_angle` error-type
  migration, **`inverse`/`apply`/`apply_to_vector` → `Result`** (W03
  `exp()`-scale consumer migration); carried by the 0.25→1.0 line.
