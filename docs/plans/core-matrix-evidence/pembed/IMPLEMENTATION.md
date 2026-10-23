# P-EMBED Implementation Record

**Branch:** `fix/pembed-target-support` from develop `609c03b`; contract
#340 (converged R3; D1(a) surgery + D2(a) smoke ELF). Catalog regenerated:
manifest hash `aae7f285` (delta expected: feature-table change), drift gate
green.

## The implementation

- **S1**: amari-core declares `num-traits`/`thiserror` directly with
  `default-features = false`; `std` wires `num-traits/std` +
  `thiserror/std`; **unconditional `libm = "0.2"`** with written
  justification (same lock version 0.2.16).
- **S2**: `phantom-types = ["num-traits/libm"]`.
- **S2b**: `src/math_dispatch.rs` — **three-arm dispatch** over 30 routed
  sites (29 census + `ceil`): hosted+std → the INHERENT method
  (bit-identical to pre-branch, glibc-backed); hosted without std → libm;
  bare metal (`target_os = "none"`) → libm. `powi` no-std arm by squaring;
  `fract` via `x - trunc(x)`; `log(x, base)` composed for non-special
  bases; `log2`/`log10` added beyond the census (discovered mid-implementation).
- **S4**: isolated smoke fixture (`smoke/`, own `[workspace]`, zero
  dev-deps): bump allocator + panic handler, constructs `GF2Matrix` and
  `VerifiedRotor`, **links a real 3,784-byte ELF** via the target's
  default rust-lld.
- **Two absolute-constant tests adapted** with disclosed ±1-ULP tolerance:
  `round5_hyperbolic_symbolic_vector_matches_scalar_bits` (channel
  equality kept EXACT; the absolute bits document glibc values),
  `apply_zero_loss_gate_is_post_exact_quotient` (same pattern). Root
  cause: pre-branch, hosted std-off rows resolved transcendentals through
  num-traits' **std-backed** (unified-defaults) impls — glibc bits; the
  surgery removes that unification, and libm is ULP-different.

## The dispatch-design arc (four iterations, all recorded)

1. feature-gated (std arm): correct for hosts, broke nothing — but the
   first sweep's 480 failures (dead-code attr misplaced onto the macro
   DEFINITION + 160 row-test failures) obscured the verdict.
2. `target_os`-gated two-way: hosted rows all-inherent — GREEN on
   stable/nightly (1280/0), but MSRV 1.89 no-default rows failed: `core`
   lacks the methods for no_std crates on 1.89 (probed; deps do NOT
   restore resolution — a no_std crate never sees std's inherent impls).
3. `rustc_1_98` cfg gate: cargo only sets `rustc_1_*` from the package's
   declared `rust-version` — never set here → libm arm active on the
   DEFAULT row → the round5 ULP failure surfaced the seam.
4. Final three-arm (hosted+std inherent / hosted-no-std libm / bare libm):
   every observed constraint satisfied; the two absolute-constant tests
   adapted at the seam.

Key mechanism discovered (fmprobe, /tmp experiments): pre-branch hosted
std-off rows' transcendental numerics were glibc-backed ONLY via unified
dependency defaults — an accident the surgery removes deliberately. The
std-off rows' tests now exercise real libm math with two disclosed ULP
tolerances.

## Verification (pinned-runner discipline, retained warm dirs)

| Gate | Result |
|---|---|
| S3: 4 combos × 3 toolchains, `build --lib` thumbv7em | **12/12 green (codegen)** |
| S4: smoke ELF | 3,784 B, rust-lld, gf2+phantom |
| Host sweep stable 320×4 | **1,280/0** |
| Host sweep nightly 320×4 | **1,280/0** |
| MSRV 320 lib checks | **320/0** |
| Supplemental 3 quadrants | 68/68/30, all green |
| Catalog drift | regenerated, delta expected, gate green |
| Default test row / clippy / fmt / root | 515/0, ok, clean, 9/0 |

## Residuals

- The two ±1-ULP test tolerances are the honest numeric seam between
  glibc-backed (std) and libm-backed (no-std hosted) dispatch arms;
  bare-metal numerics are libm-only by construction.
- P-WASM rows inherit the dispatch (inherent arm; wasm32 has std) —
  unchanged behavior, not re-swept here (M-TGT's P-WASM step covers it).
- `rustc_1_9x` intermediate compilers (1.90–1.97) are outside the
  researched matrix; the dispatch's std arm requires only the std crate,
  which every hosted rustc provides.
