# Additive Superposition on `BindingAlgebra` — Handoff for v0.24.0

**Date:** 2026-06-21
**Target:** Amari **v0.24.0** (workspace crate `amari-holographic`)
**Status:** Proposed — design ready to implement. Surfaced by the Minuet Kagome-readiness sprint (WS 6).
**Scope:** Non-breaking, additive trait change. One new method (plus a scalar helper) on `BindingAlgebra`, each with a default implementation, so existing consumers compile unchanged.
**Origin:** `docs/HANDOFF-kagome-readiness.md` §6 (Minuet); the `DenseTrace::add` recall-decay bug. This document is the upstream home for the fix, since the root cause is an Amari trait gap, not Minuet code.

---

## TL;DR

`BindingAlgebra::bundle` is the trait's only superposition method, and it does a
**softmax-weighted average**, not an additive sum. That is correct for *attention/cleanup*
(the resonator selecting among codebook items) but **wrong for accumulation** — building a
holographic memory trace `T = Σ keyᵢ ⊛ valueᵢ`. Used for accumulation, `bundle` normalises away
the growing trace: each successive item geometrically decays the earlier ones.

**Fix (additive, fits a 0.24.0 minor):** add an `superpose` (additive) method — and a `scale`
helper — to `BindingAlgebra`, each with a default implementation expressed via the existing
`to_coefficients`/`from_coefficients` trait methods. `bundle` stays as-is (it is the attention
op). Downstream, Minuet's `DenseTrace::add` switches from `bundle` to `superpose`, which
repairs a confirmed recall-quality regression (item 1 of 5 recalled as the wrong symbol at
confidence 0.11).

---

## 1. The problem (verified in Minuet)

Minuet's `DenseTrace::add` (`src/store/trace.rs`) accumulates bound pairs into a trace via:

```rust
fn add(&mut self, item: &A, weight: f64) {
    let scaled = scale_element(item, weight);
    *trace = trace.bundle(&scaled, self.beta).unwrap_or_else(|_| trace.clone());
    // ...
}
```

`bundle(beta=1.0)` on `ProductCliffordAlgebra` computes softmax weights from the operand norms
and returns `self.scale(w1) ⊕ other.scale(w2)` with `w1 + w2 = 1` — i.e. a **weighted average**.
So the trace does not grow; each `add` dilutes everything already in it:

- after 1 add: `item₁`
- after 2: `avg(item₁, item₂)` (≈ 50/50)
- after 3: `avg(avg(item₁,item₂), item₃)` → **25/25/50**
- after *n*: the first binding's weight is ≈ (½)ⁿ⁻¹

**Probe (Minuet, `ProductCliffordAlgebra<32>`, 5 stores):**

| query | stored | recall returns | confidence |
|-------|--------|----------------|------------|
| `"france"`   | 1st  | **`"spain"`** (wrong) | 0.114 |
| `"portugal"` | last | `"lisbon"` (correct)  | 0.450 |

The 2-item unit test passes; the failure appears only at ≥3 items — which is why it hid for the
whole sprint behind a CI discovery gap (Minuet's `tests/integration/` was not auto-discovered;
fixed in Minuet WS 6, PR #18). This is the **third** place the "bundle averages, not sums" fact
has bitten downstream work this sprint (also: Minuet WS 4b gradient-attribution test
construction; Minuet WS 5 `optical_store` accumulation).

---

## 2. The semantic conflation (root cause)

`BindingAlgebra` conflates two distinct operations under one method:

| Role | Operation wanted | What `bundle` does | Correct? |
|------|------------------|--------------------|----------|
| **Attention / cleanup** (resonator selecting among codebook candidates) | softmax-weighted average, bounded magnitude | softmax-weighted average | ✅ |
| **Accumulation** (building a memory trace / superposition) | additive sum, magnitude grows | softmax-weighted average | ❌ |

The two have opposite magnitude semantics: attention keeps magnitude ~constant (select among
peers); accumulation grows magnitude (the trace *is* the sum). No single operation can serve
both. The trait needs an explicit additive path.

---

## 3. Proposed trait additions

Additive only — `bundle`, `bundle_all`, and all existing methods are unchanged.

```rust
pub trait BindingAlgebra: Sized + Clone + Send + Sync {
    // ... existing methods unchanged ...

    /// Additive superposition: `self + other`, magnitude-preserving (sum grows).
    ///
    /// This is the *accumulation* operation for holographic memory traces:
    /// `T = Σ keyᵢ ⊛ valueᵢ`. Contrast with [`bundle`](Self::bundle), which is the
    /// softmax-weighted *average* used for attention/cleanup. Using `bundle` for
    /// accumulation silently decays earlier items (see the Minuet handoff).
    ///
    /// The default implementation is element-wise coefficient addition and works for
    /// any algebra whose coefficient representation is additive (all current impls).
    /// Types with a faster inherent path should override.
    ///
    /// # Errors
    ///
    /// Returns [`AlgebraError`] only if the coefficient reconstruction fails
    /// (which, for the current algebras, it never does for a valid `Self`).
    fn superpose(&self, other: &Self) -> AlgebraResult<Self> {
        let a = self.to_coefficients();
        let b = other.to_coefficients();
        // Same `Self` ⇒ same coefficient length; zip is total.
        let sum: Vec<f64> = a.iter().zip(b.iter()).map(|(x, y)| x + y).collect();
        Self::from_coefficients(&sum)
    }

    /// Scalar multiply: `factor * self`. Used for *weighted* superposition
    /// (`trace.superpose(&binding.scale(weight)?)?`). Default impl via coefficients.
    ///
    /// # Errors
    ///
    /// Same conditions as [`superpose`](Self::superpose).
    fn scale(&self, factor: f64) -> AlgebraResult<Self> {
        let scaled: Vec<f64> = self.to_coefficients().iter().map(|c| c * factor).collect();
        Self::from_coefficients(&scaled)
    }
}
```

**Why these two:** they compose to give weighted accumulation
(`a.superpose(&b.scale(w)?)?`), which is exactly what `DenseTrace::add(item, weight)` needs. No
`superpose_all` is strictly required (it is a trivial fold over `superpose`); add one later only
if a caller wants a single-allocation batch.

### Naming

`superpose` is recommended over `add`:

- It is the **VSA-literature term** for additive bundling (Kanerva: superposition = sum), so it
  reads correctly in holographic-memory contexts.
- It is unambiguously distinct from `bundle` (attention) and from the `Add` trait
  (which is about numeric addition, not algebra superposition).
- It matches the inherent `add`/`scale` primitives each type already has, without colliding
  with the `std::ops::Add` name.

If `add` is preferred for brevity, that is acceptable — the semantics are what matter. Pick one
and use it consistently; do **not** add both.

---

## 4. Per-type implementation status

Every `BindingAlgebra` impl already has the inherent primitives to override the default
efficiently — the trait change mostly *lifts* existing code to the trait surface.

| Type | Inherent `add` | Inherent `scale` | Action for 0.24.0 |
|------|----------------|------------------|--------------------|
| `ProductCliffordAlgebra<K>` | `component_add` | `component_scale` | override both (default is correct but slower) |
| `CliffordAlgebra<P,Q,R>` | (via `inner: Multivector`) | (via `inner`) | override if cheap; default works |
| `Cl3` | `add` | `scale` | override both |
| `FHRRAlgebra<D>` | `add` | `scale` | override both |
| `MAPAlgebra<D>` | `add` | `scale` | override both — **see §5 (MAP)** |

`Cl3`, `FHRR`, `MAP`, and `ProductClifford` overrides are mechanical (the inherent methods are
already `pub`). `CliffordAlgebra` wraps a `Multivector`; check whether `Multivector` exposes an
add before falling back to the coefficient default.

---

## 5. MAP: bounded vs unbounded superposition (design note)

`MAPAlgebra`'s inherent `add` is pure element-wise addition (unbounded), but its `bundle`
applies `sign()`/`soft_sign(beta)` to keep the result bipolar (bounded). This is the one place
where "what is the right superposition" has a real choice:

- **Unbounded `superpose`** (matching the other algebras): the MAP trace grows in magnitude,
  retrieval re-normalises. Consistent across the trait; matches holographic-memory semantics.
  **Recommended** — keeps the trait uniform.
- **Bounded `superpose`** (apply `sign`): stays bipolar, no magnitude growth. Diverges from the
  other algebras and breaks the "trace = Σ" invariant the Minuet `Attribution::compute_gradient`
  exactness proof relies on (sum-to-1 against the pure-sum superposition).

Recommendation: **unbounded** for `superpose` (override with plain `add`), and document that MAP
users wanting bounded accumulation should compose `superpose` + `sign` explicitly. This keeps
the trait's accumulation semantics uniform and preserves Minuet's gradient-attribution
exactness guarantee.

---

## 6. Non-breaking analysis

- **Default implementations** via `to_coefficients`/`from_coefficients` mean no existing `impl
  BindingAlgebra` block fails to compile. New methods appear on every type immediately, with
  correct (if not maximally fast) behaviour.
- **SemVer:** additive method on a non-sealed trait = minor bump. v0.23.0 → v0.24.0 is correct.
  (A new *required* method would be breaking; the defaults avoid that.)
- **Blast radius:** 12 files reference `BindingAlgebra` across `amari-holographic` + `amari-gpu`.
  None require source changes to keep compiling. `amari-gpu`'s GPU kernels operate on the
  holographic ops; if it accelerates `bundle`, a future `superpose` kernel is a natural addition
  but is **not** required for 0.24.0 (the coefficient default runs on CPU).

---

## 7. Downstream impact

- **Minuet** (`DenseTrace::add`): switch `trace.bundle(&scaled, beta)` →
  `trace.superpose(&scaled)?`. This directly repairs the recall-decay bug and lets
  `simple_memory_full_workflow` be un-`#[ignore]`d (Minuet PR #18 left it ignored with an
  evidence note pointing here). After Minuet bumps its `amari-holographic` floor to `^0.24`.
- **Minuet** (`optical_store`, WS 5): **verified NOT affected.** It accumulates via
  `OpticalFieldAlgebra::bundle(&[trace, binding], &[1.0, 1.0])`, and that `bundle` uses the
  caller-provided weights directly (no softmax) with a per-mode amplitude that *grows*
  (`amplitude = sqrt(s²+b²)`). Only `bundle_uniform` (the `1/n` variant) averages, and
  `optical_store` does not use it. So the optical path already accumulates correctly;
  no change needed there when the floor bumps. (The `BindingAlgebra` bug is specific to
  its `bundle`, which *forces* softmax weights regardless of the caller's intent.)
- **Minuet** (`Attribution::compute_gradient`, WS 4b): the exactness proof
  (`Σ attribᵢ ≈ 1` against the pure-sum superposition) assumes additive accumulation. The
  current test builds the pure-sum via `ProductCliffordAlgebra::component_add` (inherent);
  `superpose` on the trait would let it be written generically over `A`.
- **Kagome:** consumes via Minuet; no direct change. Its `MicrowaveField` delegates to
  `OpticalRotorField` (see §10).
- **amari-gpu:** optional future `superpose` kernel; not blocking.

---

## 8. Verification

1. **Unit (per type):** `superpose(a, b)` equals inherent `add` where one exists; `scale(a, w)`
   then `superpose` equals weighted add; `superpose(x, zero()) == x`;
   `superpose(a, b) == superpose(b, a)` (commutativity).
2. **Default-vs-override parity:** for each type that overrides, assert the override equals the
   coefficient-default result (guards against divergence).
3. **Capacity/SNR:** `superpose` of *n* random unit versors has magnitude ∝ √n (expected for
   holographic traces); `bundle` stays ~constant. A test asserting this growth distinguishes
   the two operations and pins the semantic split.
4. **Integration (Minuet):** after the floor bump, Minuet's `simple_memory_full_workflow` passes
   un-ignored (recall of the 1st-stored item returns the correct symbol at confidence > 0.3).

---

## 9. Scope for v0.24.0

**In scope:**
- `superpose` + `scale` on `BindingAlgebra` (default impls + per-type overrides).
- Doc comments distinguishing `superpose` (accumulation) from `bundle` (attention).
- The verification suite in §8 (items 1–3).
- CHANGELOG entry noting the semantic clarification + the Minuet bug it repairs.

**Out of scope (later):**
- A `superpose_all` batch convenience (trivial fold; add on demand).
- `amari-gpu` GPU `superpose` kernel.
- ~~`OpticalFieldAlgebra::superpose`~~ — resolved as **not needed** (§10 q1): its
  weighted `bundle` already accumulates additively.

---

## 10. Open design questions

1. **`OpticalFieldAlgebra` parity — resolved (no action needed).** `OpticalFieldAlgebra`
   (in `amari-holographic::optical`) is a *struct* with inherent methods, not a
   `BindingAlgebra` impl — and unlike `BindingAlgebra::bundle`, its `bundle` uses the
   caller-provided weights **directly** (no softmax), so `bundle(&xs, &[1.0, …])` is already
   additive accumulation. Minuet's `optical_store` relies on exactly this. **Conclusion: do
   not add `superpose` to `OpticalFieldAlgebra`**; its weighted `bundle` is correct as-is.
   The semantic split this handoff proposes is specific to the `BindingAlgebra` trait, where
   `bundle` forces softmax averaging. (If, later, callers want a *normalised* optical
   superposition, `bundle_uniform` already exists for that.)
2. **`scale` naming.** Some types' inherent scalar-multiply is named `scale`, others
   `component_scale`. The trait method should be `scale` (shorter, matches the majority);
   overrides call the inherent one regardless of its local name.
3. **Fallibility.** Proposed `-> AlgebraResult<Self>` for uniformity with the trait's error model
   and to accommodate the coefficient default. If every override is provably infallible, a later
   refactor to `-> Self` is possible but not required for 0.24.0.

---

## 11. Pointers

- Minuet handoff (origin of this work): `Minuet/docs/HANDOFF-kagome-readiness.md` §6, §9 checklist.
- Minuet WS 6 PR #18 (where the bug was surfaced + `#[ignore]`d with evidence).
- Minuet WS 4b (PR #15): `Attribution::compute_gradient` — the exactness proof that assumes
  additive accumulation.
- Minuet WS 5 (PR #16): `optical_store` — the other latent instance of the same bug.
- Trait definition: `amari-holographic/src/algebra/mod.rs:170`.
- Per-type `bundle` impls: `amari-holographic/src/algebra/{product_clifford,clifford,fhrr,map,cl3}.rs`.
