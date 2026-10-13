# CORE-W03 stage 4 — consumer migration (evidence summary)

Branch: `fix/core-w03-inverse-exp` (base develop `9633b69`; stages 1–3 in tree).
Contract: `docs/plans/2026-10-10-amari-core-w03-inverse-exp-contract.md`
(Consumer migration rules; named Ripple obligations).

## What landed

- **`amari-core/src/rotor.rs`** — removed the stage-2 `exp_or_identity`
  bridge. `Rotor::from_bivector`, `Rotor::from_multivector_bivector`,
  `Rotor::from_axis_angle`, and `Rotor::power` are now
  `Result<Self, ExpError>` and propagate `Multivector::exp()?` directly.
  `from_axis_angle` propagates because it delegates to
  `from_multivector_bivector`. Normalization fallback
  (`normalize_versor().unwrap_or(scalar(1.0))`) is unchanged — the ripple is
  error propagation only (rotor/log mathematics stays W07).
- **`amari-wasm/build.rs`** — `dispatch_exp` now returns
  `Result<(), amari_core::ExpError>`; each generated signature arm uses
  `mv.exp()?` and returns `Ok(())`.
- **`amari-wasm/src/generic.rs`** — `WasmGenericMultivector::exp()` maps the
  `ExpError` to `JsValue::from_str(&format!("exp failed: {e}"))`, matching the
  file's existing `JsValue` error style.
- **`amari-relativistic/src/spacetime.rs`** — `LorentzRotor::boost` migrates
  its `from_multivector_bivector` call to `.expect("finite rapidity pure
  bivector exponential")` and the dead `_rotor_mv` call compiles as `Result`.
- Mechanically migrated rotor callers with justified `.expect(...)`:
  `amari-core/tests/rotors.rs`, `amari-core/src/rotor.rs` tests,
  `amari-core/examples/basic.rs`, `amari-core/benches/performance_suite.rs`.

## Per-crate migrated site counts

| file | sites | migration |
| --- | --- | --- |
| `amari-core/src/rotor.rs` (API) | 3 constructors + `power()` | `-> Result<Self, ExpError>` |
| `amari-core/src/rotor.rs` (tests) | 4 `from_bivector` | `.expect` |
| `amari-core/tests/rotors.rs` | 26 `from_bivector` + 7 `from_axis_angle` + 5 `power` | `.expect` |
| `amari-core/examples/basic.rs` | 2 `from_multivector_bivector` | `.expect` |
| `amari-core/benches/performance_suite.rs` | 3 `from_bivector` | `.expect` (closure sites keep the `Result` for criterion) |
| `amari-wasm/build.rs` | 1 template → 84 signature arms (DIM ≤ 6) | arm returns `Result`, `?` |
| `amari-wasm/src/generic.rs` | 1 wrapper | maps `ExpError` → `JsValue` |
| `amari-relativistic/src/spacetime.rs` | 1 `from_multivector_bivector` | `.expect` |

All other `.exp()`/`.inverse()` sites found in the workspace operate on `f64`,
`Dual`, or other non-`Multivector` types and needed no migration. `inverse()`
keeps `Option` semantics (versor drop-in), so zero inverse sites changed.
`amari-rewrite`, `amari-discovery`, `amari-tropical`, `amari-dual`, and
`amari-gpu` were verified clean by the workspace check, not assumed.

## Expect justification categories

1. **Bounded exact Euclidean bivectors** (`Cl(3,0,0)`, `|angle| ≤ 2π`):
   `tests/rotors.rs`, `rotor.rs` tests, `benches/performance_suite.rs`,
   `examples/basic.rs`. Inputs are exact grade-2 bivectors with finite
   coefficients; the exact-dyadic/closed-form route cannot produce a
   non-representable result.
2. **Finite-rapidity pure bivector** (`amari-relativistic`): the `v < c` guard
   bounds the rapidity, and the input is a pure grade-2 bivector.
3. **Bounded rotor power** (`power` callers): the logarithm of a bounded
   rotor multiplied by a small rational exponent remains a bounded scaled
   bivector.

No `.expect` is justified by grade membership alone.

## Verification (staged recipe)

| gate | result |
| --- | --- |
| `cargo check --workspace --all-targets` | exit 0 |
| `cargo test -p amari-core --test inverse_exp_contracts` | 22 passed / 0 failed |
| `cargo test -p amari-core --lib` | 210 passed / 0 failed |
| `cargo test -p amari-core` (all targets) | all green (inc. 27 `rotors`) |
| `cargo test -p amari-wasm --lib` | 139 passed / 0 failed |
| `cargo test -p amari-relativistic` | 52 + 17 passed / 0 failed / 2 ignored |
| `cargo fmt --all -- --check` | exit 0 |
| clippy (10 touched/consumer crates, `--all-targets -- -D warnings`) | exit 0, 0 warnings |

Logs: `stage4-*.log` in this directory. Pre-existing warnings in
`amari-rewrite`/`amari-discovery` test files (unused imports, `fn var`) are
untouched by this stage and are not `-D warnings` failures for the scoped
clippy gate.

## Deviations / ambiguities resolved

1. **`from_axis_angle` also made `Result`.** The contract names the
   `rotor.rs:20–38` constructors and `power()`; `from_axis_angle` delegates to
   `from_multivector_bivector` and cannot stay infallible without an internal
   `.expect`. Propagating is the minimal faithful choice.
2. **`amari-relativistic::LorentzRotor::boost` uses `.expect` rather than
   propagating.** The function is an infallible `-> Self` with an existing
   panic-on-`v ≥ c` guard; changing it to `Result` would ripple into
   `LorentzTransformation` and many callers outside the named ripple. The
   input is a finite-rapidity pure bivector, so `.expect` is independently
   justified (category 2 above).
3. **Criterion closure return type.** `benches/performance_suite.rs` keeps the
   `Result` returned by the rotor-creation closure unmodified; only the
   non-closure setup calls `.expect`.
4. **Standalone `examples/rust/*` and `docs/audits/*` crates were not
   migrated.** They are not workspace members, so they are outside
   `cargo check --workspace --all-targets` (the stage gate). The
   `examples/rust/*` crates are additionally not independently buildable
   (`cargo check` there fails on a pre-existing manifest defect: feature
   `visualizations` references a non-optional `plotters`). `docs/audits/*`
   pins `amari-core` by path and is an audit harness rather than a workspace
   consumer; its `Rotor`/`exp` callers would need the same mechanical
   `.expect` migration if the crate is run.
