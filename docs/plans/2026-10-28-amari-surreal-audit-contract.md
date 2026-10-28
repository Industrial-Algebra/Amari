# Amari amari-surreal Audit Contract

**Date:** 2026-10-28
**Status:** DECIDED (operator choices D1–D4 recorded below)
**Branch:** `feature/surreal-audit` (from `origin/develop`)
**Scope:** amari-surreal 1,646 LOC — full file-by-file correctness audit, pulled
forward into 0.25.0 per operator directive (smallest remaining sibling after
the flynn pull-forward).

## 1. Census

| File | LOC | Role |
|---|---|---|
| `src/epsilon.rs` | 727 | Experimental rational functions in ε (feature `experimental-epsilon`) |
| `src/short.rs` | 313 | `ShortSurreal`: validated dyadic-valued short surreals; CGT bridge |
| `src/dyadic.rs` | 260 | Exact dyadic rationals `m/2^e` (BigInt numerator) |
| `src/rational.rs` | 233 | `RationalSurreal`: exact rational scalar field (surcomplex support) |
| `src/lib.rs` | 43 | Re-exports, docs |
| `src/numeric.rs` | 31 | `NumericGame` validation handle |
| `src/error.rs` | 29 | Error enum |
| `src/prelude.rs` | 10 | Prelude |

Features: `default=["std"]`, `serialize`, `experimental-epsilon`.

**Consumers (workspace):**
- `amari-wasm` — JS bridge: `from_game`, `to_game_in`, `Dyadic::new` (JS-supplied
  exponent), `RationalSurreal`, `EpsilonRational`.
- `amari-surcomplex` — `RationalSurreal` only (auxiliary scalar field).
- `amari-discovery` — probe capability over `RationalSurreal` only.
- `amari-cgt` (upstream dependency): arena, `Birthday`, `NumericGameWitness`.

`amari-surcomplex` itself is AUDITED-ELSEWHERE in the sibling programme
(no consumer), but as a downstream consumer of this crate it joins the
consumer-impact scan for any API change below.

**Baselines (this branch):** default 21/0 (0 lib + 9 + 0 + 11 + 1 doc);
`--features experimental-epsilon` 47/0; `--no-default-features` 21/0.
fmt clean; clippy `-D warnings` clean.

## 2. Findings

### S1 (P1) — `simplest_between` violates the simplicity theorem

`short.rs` `ShortSurreal::simplest_between` selects the **leftmost lattice
point above the left bound** (`floor(L)+1` at exponent 0; `floor(L·2^e)+1`
scanning upward) instead of the **minimal-birthday number** in the open
interval, in the e=0 integer selection AND both semi-finite branches.

The Simplicity Theorem (Conway): the value of `{L|R}` is the simplest number
strictly greater than all `L` and strictly less than all `R` — simplest =
smallest birthday; among fitting integers, the one closest to zero.

Counterexamples (hand-derived, reproducible against `simplest_between` /
`from_game` today):

| Cut | Returns | Correct value | Correct birthday |
|---|---|---|---|
| `{−3.5 \| −1.5}` | −3 | **−2** | 2 |
| `{−3 \| 1}` | −2 | **0** | 0 |
| `{−3.5 \| }` | −3 | **0** | 0 |
| `{ \| 1.5}` | 1 | **0** | 0 |

Proof the e≥1 scan is NOT defective: within a fixed integer cell
`(k, k+1)`, birthday of `m/2^j` is `j + const(k)`, so minimal birthday ⇔
minimal scale `j`; at the minimal scale with a fitting point, that point is
the unique odd `m` (an even `m` would be a coarser-scale point that would
have fit earlier). The defect is confined to integer selection: at e=0 the
code takes `floor(L)+1` rather than the closest-to-zero fitting integer, and
the semi-finite branches (`{L|}`, `{|R}`) ignore that the simplest number
above/below any bound crossing zero is **0**.

**Impact:** every `from_game` conversion of a user-built numeric game whose
option interval contains an integer other than the leftmost — i.e. any game
with options spanning zero or on the negative side. Exposed to JS through
`amari-wasm` `WasmShortSurreal::from_game`. Existing tests pass only because
every exercised cut (e.g. `{0|1}`, `{1|2}`, `{0|1/2}`) has
leftmost-above-L = simplest.

### S2 (P2) — epsilon exponent arithmetic silently wraps in release

`epsilon.rs` `Mul for EpsilonPolynomial`: `EpsilonExponent(exp_a.0 +
exp_b.0)` — i32 addition; debug panics, **release wraps**. Reachable from
the public API with small inputs: `monomial(1, 2_000_000_000) *
monomial(1, 2_000_000_000)` wraps to a negative exponent, silently
corrupting valuation, sign, and ordering.

### S3 (P2) — dyadic exponent saturation + allocation amplification

`dyadic.rs`:
- `Mul`: `self.exponent.saturating_add(rhs.exponent)` — when `e1+e2`
  overflows u32 the result is **silently wrong** (`Dyadic::new(n, u32::MAX)`
  is public; multiplying two such dyadics keeps the value on the wrong
  scale).
- `scaled_numer`, `checked_reciprocal`, `floor/ceil_integer` shift by
  `exponent` bits — a public exponent of `2^30` triggers ~128 MiB BigInt
  allocations per operation (DoS-amplifiable through `amari-wasm`'s
  JS-supplied `Dyadic::new(numerator, exponent)`).

### S9 (P1, R1) — `normalize()` collapses negative-exponent terms

Found by contract review R1 and independently reproduced:
`EpsilonRational` addition followed by `normalize()` uses
`self.numer.degree() == Some(0)` — the **largest** exponent — as its
"both parts are scalar constants" test. A polynomial with terms at
exponents −1 and 0 has `degree() == Some(0)` but is NOT constant, and
the scalar-folding branch drops every term except exponent 0:

```text
ε^-1 + 1  →  1        (verified: terms collapse to {0: 1})
```

Corrupts value, ordering (`ε^-1 > 1` silently lost), and division
downstream. The guard must require a single constant term:
`valuation() == Some(0) && degree() == Some(0)`.

**R2: the same degree-only defect sits in `is_one()`** — a Laurent
polynomial `1 + ε^-1` has `degree() == Some(0)` and term(0) == 1, so
`is_one()` returns true. `is_one` gates `EpsilonRational`'s `Display`
(`if self.denom.is_one()`), so a denominator `1 + ε^-1` renders as the
bare numerator — Laurent denominators display incorrectly. Fix is the
same single-term guard: `valuation() == Some(0) && degree() == Some(0)
&& terms[0] == 1`.

### S4 (P3) — `from_coefficients` index truncation

`k as i32` for coefficient vectors longer than `i32::MAX` silently wraps
negative (needs a >2 Gi-element vector — cheap to assert away).

### S5 (P3) — Display cosmetics

`EpsilonPolynomial` Display emits `1 + -1·ε` for negative coefficients.

### S6 (P3) — `RationalSurreal` naming hazard

`RationalSurreal` is an exact rational **scalar field** (1/3 is not a
short surreal this crate implements); the name and prelude re-export can
mislead. Doc comment already scopes it correctly — add a one-line "not a
surreal number" note to the type docs.

### S7 (P3) — `short_birthday` saturation

`ceil.to_u32().unwrap_or(u32::MAX.saturating_sub(self.exponent))` silently
saturates for dyadics with `|value| > u32::MAX`. Document the bound or
error honestly.

### S8 (P3) — test coverage gap

Zero in-crate unit tests under default features (the 16 lib tests live
behind `experimental-epsilon`); dyadic/short/rational exercised only by 30
thin integration tests; **no negative-cut simplicity oracles anywhere** —
the gap that let S1 ship.

**Positives recorded:** epsilon ordering logic is mathematically sound
(valuation-term sign, cross-multiplied `Eq`, denominator sign
normalization); the documented `Hash`-footgun non-implementation of `Hash`
for `EpsilonRational` is correct; `Ord`/`Eq` are consistent;
`to_game_cached` canonical reconstruction is correct; `Dyadic` hash/eq
consistency is protected by the normalization invariant in all
constructors.

## 3. Decisions (operator, ask_user 2026-10-28)

| # | Question | Decision |
|---|---|---|
| D1 | S1 fix shape | **(a) Correct selection + oracles** — e=0: closest-to-zero fitting integer (0 if it fits; ties impossible: if both −1 and 1 fit, 0 fits); semi-finite: `{L|}`→0 when L<0 else `floor(L)+1`, `{|R}`→0 when R>0 else `ceil(R)−1`; keep the (provably correct) e≥1 scan; oracle table incl. negative/zero-spanning cuts + `from_game`/`to_game_in` roundtrip properties. |
| D2 | S2+S3 safety policy | **(a) Constructor bound + checked ops — AMENDED R1** — constructor bound (`MAX_EXPONENT = 1 << 20`) via new `SurrealError::ExponentOverflow`, PLUS **output-bound enforcement in every arithmetic op**: exponent growth uses `checked_add` against `MAX_EXPONENT`; `checked_mul` (Result-returning) added for programmatic use; the infallible `Mul` ops delegate and panic with a clear `ExponentOverflow` message (honest, documented). R1 showed constructor bounds alone do not survive compounding — eleven repeated squares from `2^20` reach the i32 wrap in release; addition is safe by `max(e1,e2)` (no growth), multiplication is the growth path and is now checked. Kills wrap, saturation, and the allocation DoS on every path. |
| D3 | S8 test program | **(a) Unit + oracle suite** — in-crate `#[cfg(test)]` units for dyadic/rational/short plus the simplicity oracle table and roundtrip properties. |
| D4 | epsilon feature status | **(a) Fix in place, keep experimental** — S2 fix under the existing gate; no graduation, no API expansion. |

## 4. Remediation units

### U1 — simplicity-theorem correction (S1, D1a)

- `simplest_between`: e=0 branch selects the closest-to-zero integer in
  `(floor(L)+1 ..= ceil(R)−1)`; `(Some, None)` returns 0 when `L < 0`, else
  `floor(L)+1`; `(None, Some)` returns 0 when `R > 0`, else `ceil(R)−1`.
  e≥1 scan unchanged.
- Oracle table test: the four counterexample cuts above plus canonical
  positives (`{0|1}`→1/2, `{1|2}`→3/2, `{−1|1}`→0, `{−2|−1}`→−3/2,
  `{1/2|3/4}`→5/8, adjacent-dyadic worst cases at the `max_e+1` bound).
- Roundtrip property: for a table of user-built games (negative options
  included), `from_game` then `to_game_in` then `from_game` yields the same
  dyadic value.

### U2 — exponent safety (S2, S3, S4; D2a)

- `SurrealError::ExponentOverflow { requested, bound }` variant.
- `Dyadic::try_new(numer, exponent) -> Result<Self, SurrealError>` with
  `exponent <= MAX_EXPONENT (1 << 20)`; `Dyadic::new` becomes the
  panicking convenience wrapper (documented) delegating to `try_new`.
- `EpsilonPolynomial::monomial` / `from_coefficients` validate exponents
  against the same bound; `from_coefficients` asserts `k <= i32::MAX` (S4).
- **Output-bound enforcement (D2a as amended):** `Dyadic::checked_mul` and
  `EpsilonPolynomial::checked_mul` return
  `Result<Self, SurrealError::ExponentOverflow>` when the output exponent
  would exceed `MAX_EXPONENT`; the infallible `Mul` impls delegate and
  panic with that message. `Add` needs no check (`max(e1,e2)` cannot grow
  past the input bound). Regression test: repeated squaring from the bound
  errors/panics at the crossing multiplication instead of wrapping. The
  u32 `saturating_add` in `Dyadic::Mul` is deleted (replaced by the
  checked path).
- **R2: checked functions never panic.** `checked_reciprocal`,
  `checked_div`, and the internal `from_ratio` route through the FALLIBLE
  construction path and propagate `ExponentOverflow` as `Err` — the
  panicking surface is exactly `Dyadic::new` (documented convenience
  wrapper) and the infallible `std::ops` impls; no `Result`-returning
  function may reach a panic on any input.
- **R2: `RationalSurreal` overflow posture, stated explicitly.** The
  `BigRational`-backed layer is arbitrary-precision: addition,
  subtraction, multiplication, `checked_div`, `checked_reciprocal`, and
  all comparisons are exact and total — numeric wrap is impossible by
  representation. The only checked error is `DivisionByZero`.
  Intermediates (lcm/cross-multiplication) allocate proportionally to
  operand size; no exponent-style bound applies because the type has no
  exponent representation. Unit tests assert the posture (exact ops on
  extreme rationals; comparisons total).
- `short_birthday` (S7): document saturation in doc comment (values with
  `|value| > u32::MAX` saturate; unreachable under normal use).
- **Consumer migration (in-repo):** `amari-wasm/src/surreal.rs`
  `WasmDyadic::new` switches to `try_new` mapping the error to `JsValue`
  (no JS-triggerable panic).

### U3 — unit + oracle suite (S8, D3a)

- In-crate units: `dyadic` (normalization, arithmetic, `checked_div`
  non-dyadic error, reciprocal, birthday table: 1→1, 1/2→2, 3/4→3, 5/8→4,
  3/2→3, 5→5), `rational` (`from_ratio` zero-denom, `to_short_if_dyadic`
  both ways incl. 1/3→None), `short` (value/eq/ord, provenance retention).
- The U1 oracle table and roundtrip properties live as in-crate units
  exercising the public API.

### U4 — epsilon fixes + P3 sweep (S9, S2, S5, S6; D1–D4)

- **S9 (P1): single-constant-term guards in BOTH `normalize()`'s
  scalar fold and `is_one()`** — `valuation() == Some(0) && degree() ==
  Some(0)` (+ term == 1 for `is_one`). Regressions: `ε^-1 + 1` stays
  `ε^-1 + 1`; `ε^-2 + ε^-1 + 1` stays; `(ε^-1) + 1 > 1` orders Greater;
  a denominator `1 + ε^-1` renders `(...) / (1 + ε^-1)` (not bare
  numerator); `is_one()` false for `1 + ε^-1`, true for `1`; scalar fold
  still fires for genuine constants (`6/3 → 2`, existing tests).
- Exponent bound in `monomial`/`from_coefficients` (U2).
- **Wasm epsilon constructors (R1 P2, name corrected R2):** the actual
  JS exponent entry point is `WasmExperimentalEpsilonRational::monomial(coeff,
  exponent: i32)` in `amari-wasm/src/surreal.rs` — currently infallible
  with an arbitrary JS-supplied i32. It becomes fallible, mapping
  `ExponentOverflow` to a `JsValue` error; `checked_reciprocal` /
  `checked_div` / `compare` re-verified to keep mapping all
  `SurrealError`s to `JsValue`. No JS-reachable panic paths remain in the
  epsilon bridge.
- Display: negative coefficients render `− c·ε^e` (leading) /
  ` − c·ε^e` (subsequent), not ` + -c·ε`.
- `RationalSurreal` type docs gain the "exact rational scalar, not itself a
  surreal number of the implemented layer" note (S6).
- Everything under the existing `experimental-epsilon` gate; feature name
  unchanged. Negative-exponent support is preserved (bounds are
  symmetric on |exponent|; the R1 reviewer's framing).

## 5. Verification matrix

- `cargo +1.98.0 test -p amari-surreal` (default), `--features
  experimental-epsilon`, `--no-default-features` — all green, with the new
  units/oracles.
- `cargo +1.98.0 fmt --check`; `cargo +1.98.0 clippy -p amari-surreal
  --all-targets -- -D warnings` in both feature configurations.
- `RUSTDOCFLAGS='-D warnings' cargo +1.98.0 doc -p amari-surreal --no-deps`
  (default + no-default).
- Consumer smoke: `cargo +1.98.0 test -p amari-wasm` (surreal bridge),
  `cargo +1.98.0 check -p amari-surcomplex -p amari-discovery`.
- MSRV 1.89 build of the crate in all feature states.
- S9 regression suite: negative-exponent additions preserve terms; ordering
  with `ε^-1` vs constants is Greater; repeated-square exponent-overflow
  regression from the bound errors/panics honestly (no wrap in release).

## 6. Out of scope

- Epsilon layer graduation (stays `experimental-epsilon`, D4).
- Puiseux/Hahn exponent extensions (newtype exists for this future).
- Polynomial GCD canonicalization for `EpsilonRational` (documented
  non-goal; `Hash` correctly unimplemented).
- `amari-surcomplex`'s own audit (AUDITED-ELSEWHERE per the programme).
- Semver/publication mechanics (separate release train).

## 7. Provenance

- Audit method: full file-by-file read on `feature/surreal-audit`
  (worktree `.worktrees/surreal-audit`), baselines run in-worktree.
- S1 counterexamples derived by hand against the simplicity theorem and
  verified against the committed implementation's control flow.
- Decisions D1–D4 recorded via `ask_user` 2026-10-28 (all recommended
  options); D2 amended R1 (constructor bound alone does not survive
  compounding — output-bound enforcement added).
- S9 added in contract review R1 (reviewer-found; independently
  reproduced in-worktree: `ε^-1 + 1 = 1`, terms collapse to `{0: 1}`).
- Programme context: `docs/plans/2026-10-27-amari-1.0-sibling-audit-programme.md`
  (Phase B: surreal; pull-forward into 0.25.0 by operator directive,
  following the flynn precedent).
