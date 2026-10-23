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

## R1 remediation (2026-10-23)

Four findings, all fixed: (1) **P1 — the first ELF was hollow**: linker
GC stripped main and every consumer path (entry 0x0, no executable
sections; the "link" proved only that rust-lld emits files). Fix: the
fixture's `.cargo/config.toml` sets `--entry=main`; the rebuilt ELF has
entry `0x20779`, `.text` 0x31d8 executable bytes, and 7 retained
`amari_core` symbols — the consumer paths are real. (2) The fixture's
rotor supplied 2 coefficients where Cl(0,2,0) requires 4
(`VerifiedMultivector::new` rejects the short vector); now the correct
unit bivector `[0,0,0,1]`. (3) The absolute-constant tolerances admitted
2 ULP (`EPSILON*8` vs 8.88e-16 spacing at that magnitude); both tests
now assert exact ONE-bit distance via `to_bits().abs_diff`. (4) The
sed-derived supplemental runner still pointed at the nostd tree and had
overwritten its committed evidence (my `git add -A` baked the damage
into this branch); restored from develop, and a standalone
`supplemental.sh` (zero nostd references, OUT_ROOT_OVERRIDE hook) now
records P-EMBED results in their own directory — re-run green
68/68/30.

Re-verified after remediation: default 515/0, row1 437/0, clippy, fmt;
ELF evidence above.
