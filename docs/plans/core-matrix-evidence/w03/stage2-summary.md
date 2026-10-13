# CORE-W03 stage 2 — exp() routing + exp_series (evidence summary)

Branch: `fix/core-w03-inverse-exp` (base develop `9633b69`, stage-1 working tree).
Contract: `docs/plans/2026-10-10-amari-core-w03-inverse-exp-contract.md`
(D02.3 routing, D02.4 series, erratum 2026-10-12).

## What landed

- `Multivector::exp()` → `Result<Self, ExpError>` with the frozen precedence:
  1. exact zero → `1`; 2. exact pure scalar → `f64::exp(s)` (non-finite →
  `NonFiniteResult{ScalarRoute}`); 3. exact bivector consumed by the stage-1
  exact dyadic square (nilpotence → `1 + B` exactly; non-simple → series; simple
  → closed form, non-finite output → `NonFiniteResult{ClosedForm}`); 4. general
  multivector → `exp_series`.
- `exp_series()` per D02.4: smallest `k` with scaled coefficient norm ≤ 0.5 via
  staged power-of-two division (`2^64` chunks then `2`s), `term_n =
  term_{n-1}·x / n` (division), `term_norm > 2^-36·max(result_norm, term_peak_norm)`
  with `term_peak` seeded by the identity norm, hard cap 64 terms
  (`NoConvergence` on exhaustion), then `k` squarings (`NonFiniteResult{Squaring}`).
- Old grade-membership/`1e-10` and `b_squared`/`1e-14` closed form removed.
- Stage-1 module untouched except removal of the temporary `#[allow(dead_code)]`.
- `exp()` rustdoc carries the condition-aware `max(1e-6, C·S·u)` statement,
  `C = 2e4`, and the squaring-channel hazard.

## Verification (all under the staged recipe)

| target | result |
| --- | --- |
| `cargo test -p amari-core --lib` | 210 passed / 0 failed |
| `cargo test -p amari-core --test inverse_exp_contracts` | 19 passed / 3 failed (inverse items 1–3, stage-3 boundary) |
| `cargo test -p amari-core --no-fail-fast` (all targets) | every other target green; only inverse 1–3 fail |
| `cargo check -p amari-core --lib` | ok |
| `cargo check -p amari-core --no-default-features --lib` | ok |
| `cargo fmt -p amari-core --check` | ok |
| `cargo clippy -p amari-core --all-targets -- -D warnings` | 0 warnings |

Logs: `stage2-*.log` in this directory.

## Long-angle calibration (item 12, `0.1 + t·e12` in Cl(2,0,0))

Measured relative errors against `e^0.1 (cos t + sin t e12)`:

| `t` | coefficient scale `S` | relative error | `S·u` | `rel/(S·u)` |
| --- | --- | --- | --- | --- |
| 1e4 | 1.00000000005e4 | 1.7534e-9 | 1.1102e-12 | 1.58e3 |
| 1e8 | 1.0e8 | 1.2892e-4 | 1.1102e-8 | 1.16e4 |
| 1e12 | 1.0e12 | 3.0555e-1 | 1.1102e-4 | 2.75e3 |

`C = 2e4` covers all three under `max(1e-6, C·S·u)` (the `1e-6` floor dominates
at `t = 1e4`). The test's own acceptance is the loose `<= 0.5` condition bound,
which the frozen `2^-36` truncation plus `k`-squaring amplification saturates;
the achieved values above are frozen here.

## Deviations / ambiguities resolved

1. **Rotor API not made `Result` in stage 2.** The named rotor/consumer ripple
   (`rotor.rs` constructors + `power()`, wasm dispatch) touches ~50 call sites in
   `tests/rotors.rs` and `benches/performance_suite.rs` and is explicitly stage 4
   in the dispatch. Stage 2 keeps `Rotor`'s total signatures and routes the three
   internal `exp()` sites through a documented module-private `exp_or_identity`
   wrapper (identity fallback, matching the pre-W03 `unwrap_or(scalar(1))`
   behavior), so `amari-core` compiles and its tests are unchanged.
2. **Non-finite pure scalar.** The dispatch says the scalar route requires a
   finite scalar. Implemented as: all pure scalars take route 2; a non-finite
   `f64::exp` result is reported as `NonFiniteResult{ScalarRoute}` (rather than
   falling through to the series and reporting `SeriesScaling`). No enumerated
   test covers this.
3. **Non-finite bivector coefficients.** The exact square cannot decompose them,
   so such inputs route to `exp_series`, which reports
   `NonFiniteResult{SeriesScaling}`. No enumerated test covers this.
4. **`inverse_exp_contracts.rs` compile defect.** `crafted_dyadic_false_zero`
   moved `e` before reading it again; the file did not compile once `exp()`
   gained the `Result` API. Fixed with a non-weakening `e.clone()` on the
   residual line (no assertion changed).
5. **Catalog regen deferred.** `exp()` is a public-surface change, but the
   dispatch defers regeneration to a single pass after stage 3.
