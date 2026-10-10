# CORE-W02 — Red/Blue Evidence Summary

Branch `fix/core-w02-metric-policy` from `origin/develop` = `3dc4c31`. Lock `ba6b36cf…` throughout (`red/lock.sha256`). All gates via the validated D07 baseline recipe; zero sccache; wasm32 target not installed locally (CI covers, noted).

## Decisions implemented (D01.1–.4, maintainer ask_user 2026-10-09, recorded in the contract)

Exact `is_zero`/`Zero` + new `is_approx_zero(ε)`; norm split (`norm_squared` signed metric / new `coefficient_norm` / `norm`+`magnitude`+`abs` removed on Multivector+Vector+Bivector); coefficientwise rel+abs `approx_eq` with exact-equality/non-finite rule; exact-domain `Vector::normalize` + `Multivector::normalize` removed.

## Red (archived `red/`)

4/5 `numerical_contracts` tests failed pre-change (exact zero, null-difference equality, non-finite rule, normalize domain). Fixture corrections disclosed: the null generator in Cl(1,0,1) is index 1 — two of my initial assertions used index 0 (one caught by the stage-3 implementer, one fixed at red time).

## Staged production changes (deepseek dispatches, orchestrator-verified between stages; logs `stage*.log`)

1. `approx_eq` coefficientwise rewrite.
2. Exact `is_zero` + `is_approx_zero` (delegates to `approx_eq`).
3. `Vector::normalize` exact domain; `Multivector::normalize` removed → crate-private `normalize_versor` (signed `q>0`, no epsilon) absorbing rotor.rs (10 sites) and unicode_ops; in-crate tests updated. Implementer deviations: `&self.mv` borrow fix; `unit!` call-site migration; test renames.
4a. Norm split across 7 src files (~30 sites): coefficient_norm added ×3 types; magnitude/norm/abs removed; per-site migrations (exp convergence → coefficient_norm + CORE-W03 note; rotor axis keeps explicit `norm_squared().abs().sqrt()` + W07 note; Rotor::magnitude body → coefficient_norm). Implementer deviations A–F disclosed (rustfmt collapses; extra comprehensive_tests sites; verified.rs textual-only change in never-compiled contract; verified_laws trait rename in orphan file; macro doc fix; unrelated reformat).
4b. Integration tests migrated (audit/geometric/products/rotors); one rule-4 flag: `audit_tests.rs:229` keeps explicit metric form in `<2,0,1>`; two old-contract normalize tests deleted with pointer to numerical_contracts. `examples/basic.rs` (orchestrator miss) migrated directly.
5a–5d. Consumer waves, workspace-check-driven: dual/enumerative/holographic (holographic `normalize` → coefficient-norm conditioning, flagged); automata/fusion (geometric_ca exact-domain change: tiny nonzero now normalizes — flagged; self_assembly `.abs()` → coefficient_norm); dynamics/network (35+4 sites incl. parallel-feature module); gpu/wasm (source sites + **generator template**: build.rs now emits exact coefficient-norm guards — 84 normalize arms + magnitude arms; generated semantics flagged: wasm `magnitude`/`normalize` now coefficient-based). Wave-3 (5 bench/test sites) fixed directly by orchestrator. One deepseek connection failure retried cleanly (no partial edits).

## Green (`green/status.tsv`, `-vv` logs)

| Gate | Result |
|---|---|
| amari-core default tests, stable + nightly | **296 each** (202+6+15+1+15+6+5+17+27+2), 0 failed |
| release-mode contract suites (numerical/arithmetic/oracle) | 0 |
| warning-denied Clippy: default both compilers; no-default lib; std-only; high-precision | all 0 |
| no-default lib check; doc `-D warnings` both compilers | 0 |
| workspace all-targets check (host) | 0 — 55 files changed total |
| workspace tests | 20 suites ok; only the 16 known pre-existing local `amari-discovery` supervisor failures (0 non-supervisor) |
| workspace `fmt --check` | clean |
| amari-dynamics `parallel` feature check | 0 |
| discovery catalog | regenerated, hash `864e84a5…`, 11,077 items (surface shrank with removals) |

## Known semantic changes beyond the four decisions (all flagged by implementers, none silent)

- wasm public `magnitude`/`normalize` dispatch now coefficient-based (was `sqrt(|q|)`).
- holographic/automata conditioning sites now coefficient-based; `geometric_ca` tiny-nonzero normalization is now exact-domain.
- `adaptive.rs` submultiplicativity check now uses coefficient_norm (not provably submultiplicative in general; did not trip in any suite — recorded as a potential false-positive risk for non-Euclidean inputs).

## Limits

Closes CORE-07 (null equality) and implements D01.1–4. Inverse/exponential internals and thresholds (CORE-01/03–06), Hodge convention (CORE-08), rotor domains (CORE-11/14) remain open (W03+). Consumer migrations are compile/test-verified with per-site rules; per-site mathematical review of every consumer choice is the review loop's job, not assumed.

## Review round 1 — findings and remediation

Round 1 (report-only) returned **3 P1 / 1 P2** — all verified real, all remediated:

1. **P1 exported `unit!` broke downstream (E0624 private `normalize_versor`).** Fixed: macro expansion now uses only public API (`coefficient_norm` + `Mul<f64>` inline exact guard, normalize-or-self preserved). Operand contract now Multivector-only; the previously supported Vector operand is removed with `Multivector::normalize` (disclosed in the macro docs). New downstream-style `tests/macro_contracts.rs` covers the exported macro.
2. **P1 adaptive verification rejected correct products** (coefficient norm is not submultiplicative; reviewer counterexamples Cl(1,1,0) pair and (1+e₁)²). Fixed with the valid signature-independent Cauchy–Schwarz bound `‖ab‖₂ ≤ N·‖a‖₂·‖b‖₂` (N = BASIS_COUNT) plus small relative slack; derivation documented in-code; regression test with both counterexamples added (also asserts the old unscaled check demonstrably failed).
3. **P1 `coefficient_norm` lost representable magnitudes** (1e-200 → 0, 1e200 → inf via naive squaring). Fixed with max-scaled sum-of-squares; explicit NaN propagation (f64::max would silently ignore NaN) and inf pass-through. Red-first: `coefficient_norm_preserves_representable_extremes` failed against the old body (archived `r1-red.log`), green after.
4. **P2 missing contract coverage.** Added seven tests: extremes, non-finite policy, null-element coefficient norm, `is_approx_zero` vs `approx_eq`-against-zero, large-scale relative-component pair (1e12-scale, absolute-only would fail), `num_traits::Zero` delegation (num-traits added as dev-dependency). Compile-failure coverage of removed APIs is intentionally not trybuild-based: removals are enforced by the whole workspace (incl. examples/benches) compiling; noted for the reviewer.

Post-fix gates (`green/status.tsv` R1-* rows): amari-core stable 303 tests across the archived log (round-2 review corrected the earlier "304 both compilers" claim; the fresh nightly post-fix evidence was missing and is provided in R2). TDD guard warnings on the direct fixes reconciled: the P1-3 fix was red-first in-repo; the P1-1/P1-2 fixes' red evidence is the reviewer's independent reproduction, disclosed here.

## Review round 2 — findings and remediation

Round 2 returned **1 P1 / 2 P2** — all legitimate, all remediated:

1. **P1 `unit!` (and the wasm generator) overflowed on subnormal norms** (`1e-320` scalar → `1.0/1e-320 = inf` → NaN coefficients; reviewer-observed `[inf, NaN]`). Root cause: reciprocal-multiplication scaling. Fixed by division-based scaling: added public `Div<f64>` for `Multivector` and `&Multivector` (documented rationale), then replaced `x * (1.0/n)` with `x / n` at every W02-introduced site — `unit!` expansion, `normalize_versor`, `Vector::normalize`, holographic `CliffordElement::normalize`, geometric_ca center/product guards (both), and the wasm `build.rs` normalize arm template. Subnormal regressions added for the macro (`1e-320` scalar → exactly `1.0`) and `Vector::normalize` (`1e-160` scale; unit to subnormal precision 1e-3 — the metric square `1e-320` carries only ~11 mantissa bits, so 1e-12 unit-ness is unattainable there). Pre-existing reciprocal sites in inverse (`lib.rs:569`) and exp (`lib.rs:614`) are untouched W03 scope; their subnormal reciprocal risk is recorded for W03. Documented limitation kept explicit: the metric square becomes subnormal below ≈1.5e-154 per (single, Euclidean-signature) coefficient — normalize still succeeds there, to subnormal precision — and rounds to zero outright below ≈1e-162, returning `None`; mixed-signature or cancelling components can round to zero at any scale. That is the `norm_squared` computation, not the scaling.
2. **P2 adaptive regression test verified its own formula, not the production verifier.** Reworked: both reviewer counterexamples now flow through `AdaptiveVerifier::verify_geometric_product_properties` (constructed via in-file struct literal — no async/GPU probing) and must be accepted; the independent bound arithmetic was dropped in favor of the production path.
3. **P2 evidence record overstated counts** (304 vs 303; nightly post-fix test parity unverified). Corrected above; fresh post-round-2 evidence now archived: **305 tests across 11 suites on BOTH stable and nightly, identical test-name inventories** (`green/R2-N-default-test.log`, `r2-fix-core.log`), clippy 0 both compilers, workspace all-targets check 0, fmt clean, catalog regenerated (`aa4689c3…` — the `Div` impls are a public surface addition), lock `ba6b36cf…` unchanged.

Test-count lineage: 294 (integrated W01 baseline) → 296 (initial W02 green, stage 4b) → 303 (round-1 additions) → 305 (round-2 subnormal regressions).

## Review round 3 — findings and remediation

Round 3 returned **0 P1 / 1 P2 / 1 P3** — no production defect; both findings were overclaims in this evidence record:

1. **P2 the Vector regression did not pin reciprocal scaling** — correct: for any representable positive `q`, `1/sqrt(q)` is finite, so `Vector::normalize`'s reciprocal path never overflowed and both scaling paths carry the same ~5e-6 subnormal-q rounding (reviewer probe: old-path norm 1.0000055664551362). The test is renamed `vector_normalize_handles_subnormal_metric_square` and its comment now states exactly that, crediting the scaling-method pinning to the new test below and the macro regression. Added `div_f64_distinguishes_division_from_reciprocal_scaling`: owned and borrowed `Multivector::scalar(1e-320) / 1e-320` must equal `scalar(1.0)` exactly — reciprocal multiplication (inf) demonstrably fails this.
2. **P3 subnormal-square vs complete-underflow scales conflated** ("≲1e-154 per coefficient"). Corrected above: subnormal metric squares begin ≈1.5e-154 per single Euclidean coefficient (normalize succeeds, reduced precision); rounding to zero begins ≈1e-162; cancellation can zero q at any scale.

Post-fix gates: **306 tests / 11 suites on both stable and nightly, identical inventories** (`r3-fix-core.log`, `green/R3-N-default-test.log`), warning-denied clippy 0 on both (`green/R3-*-default-clippy.log`), fmt clean, catalog re-verified `aa4689c3…` (test-only round; hash unchanged), lock `ba6b36cf…` unchanged.

Test-count lineage: 294 (integrated W01 baseline) → 296 (initial W02 green) → 303 (round 1) → 305 (round 2) → 306 (round 3).

## Review round 4 — convergence

Round 4 returned **0 P1 / 0 P2 / 1 P3** and a readiness verdict of "ready to commit and open a PR to develop." The P3 (lineage mislabel: 296 is the initial W02 green, not the pre-W02 baseline of 294) is applied above. Loop: R1 3/1/0 → R2 1/2/0 → R3 0/1/1 → R4 0/0/1, converged at 0 P1/P2.

Verified-by-reviewer highlights: division vs reciprocal behavior exactly as documented ([1.0, 0.0] vs [inf, NaN]); IEEE-754 boundary wording (onset ≈1.49e-154, zero-rounding ≈1.57e-162); archives, inventories, catalog `aa4689c3…`, lock `ba6b36cf…` all exact.

## PR review + develop integration (PR #283 concurrent activity)

PR-level comment review posted 0 P1 / 1 P2 / 1 P3:
- **P2 catalog conflict with develop** — develop had advanced via PR #283 (amari-rewrite cohort 5 + its catalog regen from a pre-W02 tree). Resolved per catalog discipline: merged origin/develop, regenerated from the merged tree — `822baaf8…`, 11,086 items (+9 from #283's new amari-rewrite surface). amari-rewrite required no W02 migration (workspace all-targets check clean, `merge-283-workspace.log`).
- **P3 PR description omission** — `approx_eq` also treats exact equality (including ±inf == ±inf via the leading `a == b` arm) as equal; PR body amended.

Merged-tree gates: 306 tests / 11 suites stable AND nightly, clippy 0, fmt clean, lock `ba6b36cf…` unchanged (`merge-283-core-test*.log`, `merge-283-core-clippy.log`).
