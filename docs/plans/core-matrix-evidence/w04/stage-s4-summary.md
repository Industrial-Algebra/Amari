# CORE-W04 stage 4 — duals and dimension-general components (evidence summary)

Worktree `fix/core-w04-rotor-duality`, HEAD `50e9b46e` with the S1 + S2 + S3 +
S4 changes uncommitted on top. `Cargo.lock` sha256 prefix `eb2442dc`, so the
`--locked` recipe is unchanged. Scope authority:
`docs/plans/2026-10-13-amari-core-w04-rotor-duality-contract.md` §4.1–§4.4, §5,
§6, §7-S4, and the consumer companion
`docs/plans/2026-10-13-amari-core-w04-consumer-impact.md`. Target dir
`target/w04`; recipe pinned (see each `stage-s4-*.log` header):

```
RUSTC_WRAPPER='' RUSTC_WORKSPACE_WRAPPER='' RUSTC=/home/lucien/.nix-profile/bin/rustc \
  RUSTFLAGS='-C target-cpu=x86-64 -C target-feature=-avx2' CARGO_TARGET_DIR=target/w04 --locked
```

S4 delivers the permutation-only formal complement, the metric Hodge dual
(`reverse(A)·I`, degenerate rejection), the typed 3D duals, the
dimension-general bivector-component API, the migration of the `hodge_dual`
call sites, and the 3D-concrete relocation of the axis-angle/matrix helpers.

## Files

- `amari-core/src/lib.rs` — `Multivector::formal_complement` (§4.1);
  `Multivector::hodge_dual -> Result<Self, DualError>` with `R > 0` rejection
  replacing the silent metric zeroing (§4.2); deleted the generic
  `Vector::<P,Q,R>::hodge_dual`; added `impl Vector<3,0,0>` /
  `impl Vector<0,3,0>` and `impl Bivector<3,0,0>` / `impl Bivector<0,3,0>`
  typed duals (§4.3); dimension-general `bivector_blade_mask` helper,
  `Multivector::set_bivector_component -> bool`, `Bivector::zero`,
  `component`, `set_component`, `e_ij`; `from_components` / `e12` / `e13` /
  `e23` / `get` / `Index` now delegate to the general lexicographic map (§4.4);
  two-index-system rustdoc and the `compile_fail` doctest pinning the deleted
  generic dual.
- `amari-core/src/rotor.rs` — `from_axis_angle` and `to_rotation_matrix` moved
  from the generic impl to `impl Rotor<3,0,0>`; setters handle the new `bool`.
- `amari-core/src/comprehensive_tests.rs`, `amari-core/tests/audit_tests.rs`,
  `amari-core/tests/arithmetic_regressions.rs`,
  `amari-core/tests/basis_word_oracle.rs`,
  `amari-core/tests/geometric_product.rs` — migrated dual/setter call sites.
- `amari-enumerative/src/geometric_algebra.rs` — 9 `Multivector::hodge_dual`
  call sites migrated to `.expect("Euclidean GA pseudoscalar is non-degenerate")`
  (all enumerated signatures are non-degenerate; matches the existing `apply`
  migration pattern).
- `amari-relativistic/src/spacetime.rs`, `amari-calculus/src/derivative.rs`,
  `amari-gpu/src/calculus.rs` — bivector-component setters handle the `bool`
  result; the comments were corrected to the dimension-general map (4D
  components 0/1/2 are e12/e13/e14 = γ₀γ₁/γ₀γ₂/γ₀γ₃, i.e. a proper boost
  bivector).
- `amari-core/tests/rotor_duals_components_s4.rs` — new public S4 battery
  (21 tests).
- `docs/plans/core-matrix-evidence/w04/stage-s4-*.log` + this summary.

## Verification (see `stage-s4-*.log`)

| Gate | Result |
|---|---|
| `cargo test -p amari-core --lib --locked` | **234 passed, 0 failed** |
| `cargo test -p amari-core --locked` | **466 passed, 0 failed** (234 lib + 229 integration + 3 doctests; S3: 444) |
| `cargo check --workspace --all-targets --locked` | pass (only the pre-existing wgpu future-incompat warning) |
| `cargo clippy -p amari-core --all-targets --locked -- -D warnings` | pass, 0 warnings |
| `cargo fmt --all -- --check` | clean, exit 0 |
| `RUSTDOCFLAGS="-D warnings" cargo doc -p amari-core --no-deps --locked` | pass |
| `cargo check -p amari-core --lib --no-default-features --locked` | pass |
| `cargo check -p amari-core --lib --no-default-features --features std --locked` | pass |

Post-migration spot checks: `cargo test -p amari-enumerative -p amari-relativistic
-p amari-calculus --locked` — all green (enumerative 239, relativistic 52 + 1
ignored, calculus etc.).

The new `rotor_duals_components_s4` integration binary contributes 21 tests;
the doctest count rises 2 → 3 with the `compile_fail` pin.

## S4 batteries (all green)

Public API (`tests/rotor_duals_components_s4.rs`, 21 tests):

- `formal_complement`: Cl(2,0,1) `e3 ↦ e12` (CORE-08 pin); Euclidean-3D basis
  `e1↦e23`, `e2↦−e13`, `e3↦e12`, `e12↦e3`; 4D and 3D double-complement sign law
  `(−1)^{k(n−k)}` across every grade; mixed-grade additivity/homogeneity;
  grade image `k ↦ n−k`.
- `hodge_dual`: Cl(2,0,1) → `Err(DegeneratePseudoscalar)`; full Euclidean-3D
  table including `⋆e12=e3`, `⋆e13=−e2`, `⋆e23=e1`, `⋆1=e123`, `⋆e123=1`;
  Cl(2,0,0) table `e1↦e2`, `e2↦−e1`, `e12↦1`; 3D double dual = identity.
- Typed duals: `Vector<3,0,0>` / `Bivector<3,0,0>` metric and formal duals;
  `Cl(0,3,0)` typed duals exist and agree with the permutation-only formal map.
- Components: 4D component `3` round-trips `e23` (mask `0b0110`) and component
  `2` is `e14` (mask `0b1001`); 4D lexicographic table `0..5` ↔
  `e12,e13,e14,e23,e24,e34`; `Bivector::<4,0,0>::zero()`; out-of-range
  `set_component`/`set_bivector_component` return `false` with no write;
  `get` keeps the out-of-range `0.0` behavior; `e_ij(1,2)`, `e_ij(3,4)`,
  `e_ij(2,1) → Err(OutOfRange)`, `e_ij(2,2)`, `e_ij(0,2)`, `e_ij(2,5)`,
  `e_ij(4,5)`; `from_components` delegates to the general map (4D third
  component is `e14`, 3D is `e23`).

Compile-error pin (`compile_fail` doctest on `Vector`, run by
`cargo test -p amari-core --doc`):

```
test amari-core/src/lib.rs - Vector (line 1761) - compile fail ... ok
```

`grep -n "pub fn hodge_dual" amari-core/src/` lists only the four intended
definitions (`Multivector` + the four concrete 3D impls); no generic
`Vector::<P,Q,R>::hodge_dual -> Bivector` survives.

## Deviations / ambiguities resolved

1. **`Bivector::get` / `Index` index semantics.** Kept generic and delegated to
   the dimension-general map rather than restricting them to 3D-concrete impls:
   in 3D the mapping `0↦e12`, `1↦e13`, `2↦e23` is byte-for-byte the historical
   behavior (including out-of-range `0.0` for `get` and a panic for `Index`),
   and in higher dimensions the old out-of-range behavior was already broken
   (`get(3) = 0.0`). The hard-coded `[3,5,6]` array is gone entirely, so the
   literal layout no longer appears in generic code; only the lexicographic map
   remains.
2. **`Bivector::from_components` must stay generic** (used by
   `amari-automata/src/self_assembly.rs` in a generic `Bivector<P,Q,R>`
   context), so it delegates to the general map. Consequence: in 4D its third
   argument writes `e14` rather than the old hard-coded `e23`. No consumer
   passes a nonzero third argument outside 3D, so there is no observable
   regression; documented on the method.
3. **Relativistic boost bivector.** `SpacetimeAlgebra = Multivector<1,3,0>` is
   4D, so components 0/1/2 change from the old `e12,e13,e23` to the general
   `e12,e13,e14`. This is the *correct* spacetime boost bivector
   (`γ₀γᵢ`, all mixed timelike-spacelike), so the migration fixes the previous
   approximation instead of breaking it; the comments were corrected.
4. **Compile-error pin vehicle.** No `trybuild`/compiletest infrastructure
   exists, so the CORE-09 deletion is pinned with a `compile_fail` doctest on
   the `Vector` type plus a header note in the S4 battery and the grep
   verification above.
5. **`hodge_dual` migration error handling in enumerative.** All enumerated
   signatures (`Cl(3,0,0)`, `Cl(3,1,0)`, `Cl(4,1,0)`, `Cl(4,0,0)`) are
   non-degenerate, and the crate already used `.expect(...)` for the `apply`
   migration, so the nine sites use `.expect("Euclidean GA pseudoscalar is
   non-degenerate")` rather than adding a `DualError → EnumerativeError`
   conversion.
6. **Setters.** `set_bivector_component` / `set_component` now return `bool`;
   test call sites use `assert!`, library call sites use `let _ =`
   (the values are always in range) with comments corrected to the general map.

## Out of scope / noted

- `docs/audits/core-foundations-repro/tests/foundations.rs` still references the
  old `Vector::<4,0,0>::e1().hodge_dual()` API. It is a standalone crate
  (`docs/audits/core-foundations-repro/Cargo.toml`) that is not a workspace
  member, so `cargo check --workspace` does not build it; it is the historical
  audit reproduction and was left untouched per the plan's file scope.
- A workspace-wide `cargo clippy -- -D warnings` currently fails in
  `amari-tropical/src/verified.rs` (`needless_range_loop`), which is pre-existing
  and untouched by S4; the S1–S4 clippy gate is scoped to `amari-core`, per the
  recipe.
