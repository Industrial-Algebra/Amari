# CORE-W04 stage 2 — domain-aware logarithm and powers (evidence summary)

Worktree `fix/core-w04-rotor-duality`, HEAD `50e9b46e` with the S1 + S2 changes
uncommitted on top. Scope authority:
`docs/plans/2026-10-13-amari-core-w04-rotor-duality-contract.md` §2, §5, §7-S2,
and the consumer companion `docs/plans/2026-10-13-amari-core-w04-consumer-impact.md`.
Target dir `target/w04`; recipe pinned (see each `stage-s2-*.log` header):

```
RUSTC_WRAPPER='' RUSTC_WORKSPACE_WRAPPER='' RUSTC=/home/lucien/.nix-profile/bin/rustc \
  RUSTFLAGS='-C target-cpu=x86-64 -C target-feature=-avx2' CARGO_TARGET_DIR=target/w04 --locked
```

S2 delivers the domain-aware element logarithm, exact integer powers, the
non-integral `power` route, and the law batteries. Slerp/`from_vectors`/reflect
(S3) and duals/components (S4) remain out of scope.

## Files

- `amari-core/src/exact_square.rs` — no_std import repair (`use alloc::vec;`
  `use alloc::vec::Vec;`, fixing the pre-existing `--no-default-features`
  failure), plus the CORE-W04 §2c range-safe scalar machinery:
  `ExactPositive` (`from_f64`, `mul`, `add`, `sub`, `ln`, `ln_half`,
  `ln_ratio`), `ExactSigned` (`ratio_to_f64`, `cmp_abs`), `signed_order`,
  `exact_reverse_norm_positive` (the exact dyadic `N = R·R†` for the hyperbolic
  gate and evaluation), and `exact_reverse_norm_scalar_sign` (the §2d
  binary-exponentiation `N ≤ 0` gate).
- `amari-core/src/rotor.rs` — `logarithm() -> Result<Multivector, RotorError>`
  with the §2a–§2c routing; `power_integer(n)` (`0` identity / `1` bit-exact
  clone / `-1` direct inverse / certified class `exp(n·log R)` / uncertified
  binary exponentiation); `power(t)` (integral `i64` delegation, else
  `exp(t·log R)`); `binary_power` (left-to-right square-and-multiply) and
  `checked_compose`; the reordered `normalize_and_certify` output-`N` gate.
  S2 private-stored-field unit batteries.
- `amari-core/tests/rotor_log_powers_s2.rs` — the new public-API S2 law
  battery (15 tests).
- `amari-core/tests/rotors.rs` — `test_rotor_logarithm` migrated to the
  `Result` logarithm.

## Verification (see `stage-s2-*.log`)

| Gate | Result |
|---|---|
| `cargo test -p amari-core --lib --locked` | **231 passed, 0 failed** (S1: 223; +8 S2 unit batteries) |
| `cargo test -p amari-core --locked` | **420 passed, 0 failed** (231 lib + 187 integration + 2 doctests) |
| `cargo check --workspace --all-targets --locked` | pass (only the pre-existing wgpu future-incompat warning) |
| `cargo clippy -p amari-core --all-targets --locked -- -D warnings` | pass, 0 warnings |
| `cargo fmt --all -- --check` | clean, exit 0 |
| `RUSTDOCFLAGS="-D warnings" cargo doc -p amari-core --no-deps --locked` | pass |
| `cargo check -p amari-core --lib --no-default-features` | pass (pre-existing 7-error `vec!`/`Vec` failure repaired) |
| `cargo check -p amari-core --lib --no-default-features --features std` | pass |

The integration battery breakdown is in `stage-s2-core-all-tests.log`; the new
`rotor_log_powers_s2` binary contributes 15 tests.

## S2 batteries (all green)

Public API (`tests/rotor_log_powers_s2.rs`):

- boost fixture `from_bivector(e12, 1)` stored
  `[1.127625965206381, −0.5210953054937475]`; `power_integer(1)` bit-exact.
- 2π rotor `power_integer(1)` bit-exact clone including the residual bivector.
- `power_integer` `{0, 1, −1}` laws; integral `power` delegates.
- `power_integer(−2)` scalar within 2 ulp of the exact `cosh(1)` oracle.
- small-angle `1e-8` bivector preserved by the `atan2` route.
- elliptic `exp(log R) ≈ R` (≤ 8 ulp) and `log(exp(2π e12)) = 0` principal
  branch.
- hyperbolic small boost `R = 1 + 2⁻⁵⁴e12` rapidity exactly `2⁻⁵⁴`.
- hyperbolic `exp(log R) ≈ R` (≤ 8 ulp).
- uncertified multi-plane log → `Err(Uncertified)`; binary chain bit-exact for
  exponents 2 and 3; composition law `power_integer(3)∘power_integer(4) ≈
  power_integer(7)` within the fixture budget.
- non-integral `power(0.5)` matches the half-angle construction; uncertified
  log errors propagate directly.

Private stored field (`src/rotor.rs` unit batteries):

- scalar branches: `log(1) = 0` exactly; `log(4) = 2·ln2`; `±0` →
  `NotALogDomain`; `w < 0` → `BranchAmbiguous`; non-finite →
  `NotALogDomain`.
- negative sheet `−5/4 + (3/4)e12` → `Err(NotALogDomain)`.
- Cl(6,0,0) `(1+I)/√2` → `Err(Uncertified)`.
- elliptic `w = β = 1e154` finite scale `354.944677911363` (no `f64` `w²+β²`).
- hyperbolic `w = 1e308, β = nextafter(w,0)` finite rapidity
  `18.42172174950956`.
- exact mixed fixture `2²⁷ + 2²⁷e13 + e12` (Cl(2,1,0)) rapidity `28·ln2`.
- `R^32` dyadic chain `[2³¹, 2³¹]`: `inverse`/`apply`/`power_integer(32)` all
  `Err(Degenerate)`.
- R8 round-trip witness `w = 1, b = nextafter(1,0)`: exact scale
  `−18.021826694558577`, rapidity `18.714973875118524`, and
  `exp(log R) = [1.000000000000002, 1.0000000000000018]` (9/17 ulp from `R`).

## Deviations / ambiguities resolved

1. **Hyperbolic large-`q` evaluation rounding.** The contract's "`ln q` via the
   rational's mantissa/exponent decomposition" is implemented as
   `ln(m_num) − ln(m_den) + (e_num − e_den)·ln2` (`ExactPositive::ln_ratio`)
   rather than `ln(num) − ln(den)`. The two separate `ln` calls accumulate a
   ~1-ulp error in the dominant `e·ln2` term; the combined form makes the R8
   witness rapidity correctly rounded (`0x4032b708872320e2`), reproducing the
   stated `[1.000000000000002, 1.0000000000000018]` result. The small-`δ`
   branch is unchanged (`½·ln1p(δ)`).
2. **[SUPERSEDED by R1-remediation F5: `checked_compose` now requires the
   full positive-scalar exact `R·R†` certificate; rounded multi-plane
   composites return `Err(Degenerate)`.] Binary-exponentiation `N ≤ 0` gate.**
   `checked_compose` used `exact_reverse_norm_scalar_sign` — the sign of the
   exact scalar part of `R·R†`, ignoring non-scalar rounding residue — for the
   contract §2d "`N ≤ 0` detection at each checked boundary". The product of two
   uncertified multi-plane versors has a near-1 metric norm but a non-scalar
   exact `R·R†` from coefficient rounding, so the scalar-only certificate of
   `exact_reverse_norm` does not apply. The certified closed-form routes
   (`power`/`power_integer`/constructors) already used the exact scalar-`N` gate
   and returned `Err(Degenerate)`, which is what detects the `R^32` collapse.
3. **[SUPERSEDED by R1-remediation F5: the *rounded* multi-plane fixture
   `r12(0.3)∘r34(0.7)` has a non-scalar exact `R·R†` and is rejected; the
   surviving binary-fallback fixture is the exactly-representable
   `(5/4+3/4e12)(5/4+3/4e34)` with exact scalar `N = 289/64` (R2-F13).]**
   Non-simple multi-plane construction. `from_bivector` on a *non-simple*
   bivector (e.g. Cl(4,0,0) `e12+e34`, `B² = −2 + 2e1234`) returns
   `Err(Degenerate)` because the series-route output's exact reverse norm is
   non-scalar. The multi-plane binary-exponentiation fixture was therefore the
   product of two orthogonal simple rotors (`r12.compose(&r34)`), which has the
   required grade-4 content (`log → Uncertified`).
4. **Mixed-component fixture placement.** The exact `2²⁷ + 2²⁷e13 + e12`
   fixture is a unit test with direct stored-field construction. The
   exp-then-normalize route does not produce it: a 1-ulp difference between the
   printed `w` and `B₁₃` makes the metric `N` ≈ 4 instead of 1, so
   `normalize_versor` divides by ~2 and changes the rapidity.
5. **[SUPERSEDED by R1-remediation F5: the rounded six-ulp composition
   battery was deleted; the surviving law battery is the exact-dyadic
   `power_integer(3)∘power_integer(4) == power_integer(7)` identity
   (R2-F13).]** Binary exponentiation chain / composition budget. The
   exponent-`n` chain is left-to-right square-and-multiply (`R², R³, R⁶, R⁷`
   for 7; 4 compositions). The composition-law battery's left side performs 5
   compositions (2 for 3, 2 for 4, 1 for the final compose), so `m = 5` and the
   contract's `m + 1 = 6` ulp budget on the max-abs coefficient norm was used;
   the assertion was expressed in absolute terms scaled by `ulp(max_abs)`
   because coefficient-wise ulp on near-zero coefficients is not the contract's
   stated metric.
6. **Odd-grade content.** Any nonzero non-grade-0/2 coefficient (including odd
   grades, which a constructed even rotor cannot carry) is routed to
   `Err(Uncertified)`, a safe superset of the contract's "grade ≥ 4".
7. **`power_integer` error propagation.** Only `Err(Uncertified)` triggers the
   binary fallback; other logarithm domain failures (e.g. `BranchAmbiguous`)
   propagate directly, matching the contract's "certified log class → `exp`;
   uncertified multi-plane → binary exponentiation".
8. **`normalize_and_certify` ordering.** The exact stored reverse norm is
   certified *before* `normalize_versor`, so the representability limit
   (`cosh(32·ln2)` → stored `N = 0`) surfaces as `Err(Degenerate)` rather than
   `Err(NotNormalizable)`. Simple-rotor closed forms still have exactly scalar
   positive `N`, so their construction is unchanged.
