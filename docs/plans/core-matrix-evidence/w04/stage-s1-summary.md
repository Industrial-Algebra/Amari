# CORE-W04 stage 1 — errors, privacy, and the checked rotor inverse (evidence summary)

Worktree `fix/core-w04-rotor-duality`, lock `eb2442dc`. Scope authority:
`docs/plans/2026-10-13-amari-core-w04-rotor-duality-contract.md` §1.1–§1.5 and
§7-S1, with the consumer companion
`docs/plans/2026-10-13-amari-core-w04-consumer-impact.md` §C1/C3/C7.
Target dir `target/w04`; recipe pinned (see each `stage-s1-*.log` header).

S1 delivers the error surface, the private wrapper representation with checked
`TryFrom`, the deletion of the silent normalization fallbacks, the certified
scalar-`N` gate on `inverse`/`apply`/`apply_to_vector`, and the constructor
output-`N` gates. Logarithm/power *math* (S2), slerp/reflect semantics (S3),
and duals/components (S4) are explicitly out of scope.

## Files

- `amari-core/src/error.rs` — `RotorError` (16 variants, lossless
  `From<ExpError>`), `DualError`, `GradeError`, `ComponentError`; single
  cfg-independent definitions with manual `Display` and `std::error::Error`
  under `std`.
- `amari-core/src/lib.rs` — re-exports the four new error types; makes
  `Scalar`/`Vector`/`Bivector.mv` private; adds `as_multivector`,
  `into_multivector`, `try_from_multivector`, `TryFrom<Multivector>` (exact
  foreign-grade predicate); removes the projecting `from_multivector`
  constructors (replaced internally by crate-private `from_grade_projection`).
- `amari-core/src/exact_square.rs` — `exact_reverse_norm` + `ReverseNorm`:
  exact `N = R·R†` certificate via the W03 `ExactPower`/`mul_exact` machinery,
  scalar-positivity classification, single correctly-rounded `f64`.
- `amari-core/src/rotor.rs` — `from_bivector`, `from_multivector_bivector`,
  `from_axis_angle` → `Result<_, RotorError>`; `from_multivector_bivector`
  grade-2 validation → `NotABivector`; `inverse`/`apply`/`apply_to_vector` →
  `Result<_, RotorError>` with the certified-`N` gate; `compose` is now the
  raw infallible geometric product; `power` fallback deleted and error type
  migrated; `from_vectors`/`from_vectors_mv` `Option` → `Result` with the S1
  placeholder path (`TODO(S3)`); negative-sheet inverse unit battery.
- Consumers migrated: `amari-automata/src/self_assembly.rs`,
  `amari-discovery/src/probes/network.rs` + `tests/probe_network.rs`,
  `amari-gpu/tests/*`, `amari-network/{src,tests,examples}`,
  `amari-relativistic/src/spacetime.rs`,
  `amari-enumerative/src/geometric_algebra.rs`, plus amari-core's own
  tests/benches/examples and the `comprehensive_tests` internal callers.
- `amari-core/tests/rotor_error_privacy_s1.rs` — new S1 red battery (11 tests).

## Verification (see `stage-s1-*.log`)

| Gate | Result |
|---|---|
| `cargo test -p amari-core --lib --locked` | **223 passed, 0 failed** |
| `cargo test -p amari-core --locked` | **397 passed, 0 failed** (223 lib + 172 integration + 2 doctests) |
| `cargo check --workspace --all-targets --locked` | pass (only the pre-existing wgpu future-compat warning) |
| `cargo clippy -p amari-core --all-targets --locked -- -D warnings` | pass, 0 warnings |
| `cargo fmt --all -- --check` | clean |
| `RUSTDOCFLAGS="-D warnings" cargo doc -p amari-core --no-deps --locked` | pass |

Consumer spot-checks: `amari-network` 16 passed, `amari-relativistic` 71
passed, `amari-automata` 55 passed, `amari-discovery --test probe_network` 8
passed. `amari-discovery` `probes::supervisor` unit tests fail on this host
with `Os { code: 2, NotFound }` when spawning a worker binary; that failure is
environmental and independent of W04.

## S1 red batteries (all green)

- `dyadic_boost_base_is_exactly_representable` — `5/4 + (3/4)e12` stored exact.
- `r32_boost_chain_is_degenerate_for_inverse_and_apply` — five raw squarings
  store `[2³¹, 2³¹]`, `N = 0`; `inverse`/`apply` → `Err(Degenerate)`.
- `q_plus_times_q_minus_is_exact_zero_rotor` — `Q₊·Q₋ = 0`; `inverse` →
  `Err(Degenerate)`.
- `angle_one_boost_stored_inverse_matches_exact_stored_element_inverse` —
  stored `[1.127625965206381, −0.5210953054937475]` → inverse
  `[1.1276259652063807, 0.5210953054937474]`.
- `from_multivector_bivector_rejects_a_vector` → `Err(NotABivector)`.
- `from_multivector_bivector_rejects_nan_in_a_foreign_grade`.
- `vector_try_from_rejects_mixed_grades_and_nan_foreign_grades` (and `±0.0`
  accepted; borrowing form agrees).
- `scalar_and_bivector_try_from_share_the_foreign_grade_predicate`.
- `escape_round_trips_preserve_coefficients`.
- `from_vectors_s1_precedence_is_not_a_vector_then_null`.
- `exp_error_carrier_is_lossless`.
- Unit battery `test_negative_sheet_stored_inverse_uses_reverse_over_norm`:
  `−5/4 + (3/4)e12` → inverse `[−1.25, −0.75]`.

## Deviations / ambiguities resolved

1. **Negative-sheet inverse battery placement.** The store cannot produce
   `−5/4 + (3/4)e12` through any public constructor (it is on the other Spin
   component; Cl(1,1,0) has no exact `−1` rotor because `sin π ≠ 0` and no
   Euclidean plane). The battery is therefore a unit test in
   `amari-core/src/rotor.rs` where the private stored field is reachable.
   All other S1 batteries are in `tests/`.
2. **Scope of signature migration.** Per the explicit S1 prohibition,
   `logarithm`, `power_integer`, `slerp`, `reflect`, `reflect_line`,
   `hodge_dual`, and the component setters keep their S3/S4 signature stage
   here. `power` alone changed (`Result<_, RotorError>`) because S1 explicitly
   deletes its normalization fallback and `ExpError` has no normalization
   carrier; its existing math is unchanged. `from_vectors` was migrated per
   S1 item 5 with the placeholder `TODO(S3)` route.
3. **Exact-`N` division precision (superseded by R1-F2).** This stage
   originally divided by `N` pre-rounded once to `f64` (`R†/N_f64`). R1-F2
   replaced that with exact rational `R†ᵢ/N` division rounded once per
   coefficient, so the contract's "≤ 1 rounding per coefficient" now holds
   unconditionally: the `from_bivector(e12, 0.3)` Cl(1,1,0) fixture with
   stored `N = 0.9999999999999999` now returns the correctly rounded inverse
   `[1.0112711095766704, 0.15056313315161265]`. The angle-1 boost pin
   `[1.1276259652063807, 0.5210953054937474]` is unchanged.
4. **`apply_to_vector` projection.** The result is projected to grade 1 via
   the crate-private `from_grade_projection` (the removed public projecting
   constructor's behavior), documented as the §1.4 defense-in-depth.
5. **`from_axis_angle` axis failures.** Null axis → `Err(NullVector)`,
   negative-square/non-finite axis → `Err(NotNormalizable)`; the former
   silent `unwrap_or(axis.clone())` fallback is deleted.
