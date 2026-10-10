# CORE-D01 Decision Record + CORE-W02 Contract — Metric/Zero/Equality/Normalization

> **REQUIRED SUB-SKILL:** executing-plans after explicit approval of this contract.

**Goal:** Give `amari-core` honest numerical-query contracts: exact zero, separated metric/coefficient magnitudes, coefficientwise approximate equality, and exactly-checked normalization domains — resolving CORE-07's null-equality defect and the D01 ambiguity that W01 deliberately deferred.

**Baseline:** branch `fix/core-w02-metric-policy` from `origin/develop` = `3dc4c31` (W01 integrated; verified identical tree). Site lines below are current: `is_zero` 474, `norm_squared` 479, `norm` 507, `approx_eq` 517, `normalize` 522, `Zero` impl 813; `Vector::{normalize 982, norm_squared 989, norm 1004}`.

## D01 decisions (maintainer, 2026-10-09, ask_user — all recorded verbatim)

1. **Zero — exact + explicit approx.** `is_zero()` and the `num_traits::Zero` impl become exact: every coefficient `== 0.0`. New `is_approx_zero(epsilon: f64)` provides the tolerance query for callers who want one. Breaking and trait-correct.
2. **Norm — split names.** `norm_squared()` remains the **signed metric quadratic form** (documented; no sqrt). New `coefficient_norm() -> f64` = `sqrt(Σ cᵢ²) ≥ 0` (magnitude of the coefficient vector, signature-independent). Blanket `norm()` is **removed** from `Multivector` and `Vector` (breaking) — callers choose the quantity they mean.
3. **approx_eq — coefficientwise relative+absolute.** `∀i: |aᵢ−bᵢ| ≤ ε·max(1, |aᵢ|, |bᵢ|)` with caller-supplied `ε`; if any coefficient on either side is non-finite, the result is `false` unless the two are exactly equal (bit-identical). Null differences no longer vanish.
4. **normalize — exact domain checks.** `Vector::normalize` returns `Some` only when `norm_squared() > 0.0` exactly (negative-square and null vectors → `None`; no epsilon). `Multivector::normalize` is **removed** — versor normalization belongs to the inverse/rotor domain decision (D02/W03).

These are correctness-first breaking changes. Consumers relying on the old behaviors get investigated, not silently preserved.

## Scope

**Production edits (`amari-core/src/lib.rs` primarily):**
- `is_zero()` → exact; `Zero` impl delegates exact; add `is_approx_zero(ε)`.
- Add `coefficient_norm()`; remove `norm()` on `Multivector` and `Vector`; `norm_squared()` docs state signed metric semantics.
- `approx_eq` → coefficientwise rel+abs with the non-finite rule.
- `Vector::normalize` → exact-domain `Option`; remove `Multivector::normalize`.
- **Internal migrations:** every in-crate use of the removed/changed APIs (exp convergence at ~587 and dispatch guard ~561, rotor paths, verified/generic modules as found) migrates to the semantically correct named operation — each site's replacement justified in the PR body. The exponential's structural misdispatch (CORE-03..05) remains **open W03 findings**; W02 only renames what those sites compute, choosing the honest quantity (difference-magnitude convergence → `coefficient_norm`).
- `1e-14` sites at 524/536 (normalize/inverse thresholds): normalize's is removed by the domain rewrite; **inverse (~532–536) is untouched — D02/W03**, including its zero-divisor acceptance (CORE-01).

**Tests (red-first, new `amari-core/tests/numerical_contracts.rs`):** exact zero semantics (tiny-but-nonzero → `is_zero()==false`, `is_approx_zero(tiny)==true`); `Zero` trait exactness; `coefficient_norm` correctness incl. null vectors; `approx_eq` rejects `null ≈ zero` (CORE-07), accepts scaled pairs within rel tolerance, non-finite rule; `Vector::normalize` domain (+ / − / null squares); removals verified by compile-failure of the old names.

**Consumers:** workspace must compile and test green. Mechanical migrations of removed-API call sites (`norm()`, `Multivector::normalize`) map each site to `norm_squared`/`coefficient_norm`/domain-checked alternatives with a one-line justification; semantically suspicious sites become recorded findings, not silent choices. Regenerate the discovery catalog (mechanical; any amari-core surface change requires it).

**Out of scope:** inverse/exponential algorithms and thresholds (D02/W03), rotor/logarithm domains (D03/W07), Hodge conventions (CORE-08), typed wrappers (W06), any epsilon policy beyond the explicit `ε` parameters above.

## Execution process

As W01: red-first archive under `docs/plans/core-matrix-evidence/w02/red/`; staged deepseek dispatches (queries → norm split → approx_eq → normalize/removals → internal migrations → consumer migrations) with orchestrator verification between stages; green gates = full W01 parity matrix (both compilers, minimal configs, release-mode contract tests, rustfmt, doc, workspace tests with the known pre-existing local supervisor exclusions re-verified against baseline); discovery catalog regen; reviewer loop (`gpt-6-astra`, report-only, max 6, converge 0 P1/P2); PR `fix/core-w02-metric-policy` → `develop`, no self-merge.

## Authorization boundary

This document records maintainer decisions D01.1–.4 (settled) and proposes the bounded execution contract; **approval below authorizes execution only** — not W03+, not D02 inverse domains, not release actions.
