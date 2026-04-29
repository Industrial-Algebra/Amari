# amari-tropical Ordinal Substrate Design

## Purpose

This document defines the intended ordinal substrate for `amari-tropical` in the `0.21.0` cycle.

The motivating downstream use case is an optimization layer with:

- ordinals in Cantor normal form
- arena interning
- `max` as additive combination
- ordinal addition as multiplicative composition
- valuation maps given by leading-exponent extraction

This is not fully served by the current float-only `TropicalNumber<T>` design, so the ordinal substrate should be treated as a dedicated new layer.

---

## Why the Current Float Layer Is Not Enough

Current `amari-tropical` assumes a carrier like:

- `TropicalNumber<f64>`

with operations:

- `⊕ = max`
- `⊗ = standard addition`
- zero = `-∞`
- one = `0`

The ordinal substrate instead needs:

- a non-float carrier
- canonical structural comparison
- ordinal addition as composition
- valuation into leading exponents
- interning for sharing and fast identifiers

So this cannot be modeled as only a new constructor on `TropicalNumber<T>`.

---

## Mathematical Model

### Ordinal scope

For `0.21.0`, the supported ordinals should be restricted to:

- ordinals below `ε₀`

This means every ordinal can be written in canonical Cantor normal form:

`ω^e1 * c1 + ω^e2 * c2 + ... + ω^en * cn`

with:

- `e1 > e2 > ... > en`
- each `ei` itself an ordinal below `ε₀`
- each `ci` a positive natural number

This scope is expressive enough for serious structured ranking while keeping algorithms honest and implementable.

---

## Semiring Carrier

### Important point: ordinals alone are not enough

If the tropical-style additive operation is `max`, then the semiring needs an additive identity that behaves like a bottom element.

But ordinal `0` does **not** work as that bottom/annihilating element for composition, because:

- `0 + α = α`

So the optimization-facing carrier should be an extension of ordinals with an explicit bottom.

### Recommended carrier shape

Conceptually:

- `Bottom`
- `Ordinal(OrdinalId)`

Possible public naming:

- `OrdinalWeight`
- `OrdinalSemiringValue`

Recommended semantics:

- semiring zero = `Bottom`
- semiring one = ordinal zero
- `a ⊕ b = max(a, b)`
- `a ⊗ b = a + b` using ordinal addition, with bottom-annihilation

This gives the expected behavior for optimization-style accumulation.

---

## Public API Shape

### Arena and identifier

Recommended public core types:

- `OrdinalArena`
- `OrdinalId`

Although the downstream concept is “interned as NodeId”, a semantically named `OrdinalId` is preferable as the public API because it is self-describing and avoids ambiguity with unrelated graph-like node identifiers elsewhere in the workspace.

### Structural term representation

Recommended structural building blocks:

- `CnfTerm { exponent: OrdinalId, coefficient: u64 }`
- internal canonical node = descending list of CNF terms

### Optimization-facing wrapper

Recommended carrier wrapper:

- `OrdinalWeight`

with:

- `bottom()`
- `one()`
- `oplus(..., arena)`
- `otimes(..., arena)`
- valuation helpers

Because the carrier is arena-backed through `OrdinalId`, semiring-style composition should be exposed through arena-aware methods rather than assuming a completely context-free trait implementation.

---

## Representation Strategy

### Arena-backed interning

Ordinals should be stored in an interning arena:

- canonical node storage
- structural deduplication
- cheap `OrdinalId` copying
- memoization opportunities for comparison/addition/formatting

### Canonical invariants

Each interned ordinal node should maintain:

- terms sorted by strictly descending exponent
- positive coefficients only
- adjacent equal exponents merged
- zero represented canonically, not as an empty malformed variant with hidden meaning

### Suggested zero representation

A dedicated zero ordinal should exist in the arena and be representable by an `OrdinalId`.

This is distinct from semiring `Bottom`.

---

## Core Operations

### Construction

Needed public constructors/helpers:

- zero ordinal
- finite naturals
- `ω`
- construction from canonical CNF terms
- small named helpers where useful for examples/tests

### Comparison

Ordinals below `ε₀` have a total order. Comparison should be supported directly on `OrdinalId` values through the arena.

Expected comparison strategy:

- lexicographic comparison of CNF terms
- compare leading exponents first
- if equal, compare coefficients
- then continue through the tail

### Ordinal addition

Ordinal addition must follow standard ordinal, not natural/Hessenberg, addition.

Key behavior examples:

- `1 + ω = ω`
- `ω + 1 > ω`
- `ω^2 + ω + 1 + ω = ω^2 + 2ω`

Expected algorithm sketch for `α + β`:

1. if `β = 0`, return `α`
2. inspect the leading exponent of `β`
3. discard from `α` every suffix term whose exponent is strictly smaller than that leading exponent
4. if the remaining tail of `α` ends with the same exponent, merge coefficients
5. append the remaining CNF structure of `β`
6. intern the canonical result

This should be implemented with canonicalization guarantees rather than exposing partially normalized intermediate forms.

### Leading valuation

The downstream use case explicitly needs a valuation map by leading exponent extraction.

Recommended API:

- `leading_exponent(id: OrdinalId) -> Option<OrdinalId>`
- `leading_term(id: OrdinalId) -> Option<(OrdinalId, u64)>` if useful
- `valuation(weight: OrdinalWeight) -> Option<OrdinalId>` or equivalent wrapper-level helper

This should be constant-time or near-constant-time from the first CNF term.

---

## Formatting and Inspection

The ordinal substrate should have strong formatting support.

Recommended display behavior:

- `0`
- `1`, `2`, ...
- `ω`
- `ω + 1`
- `ω^2 + 3ω + 5`
- recursive exponent formatting such as `ω^(ω + 1)` where needed

Good formatting is especially important because this layer is intended for optimization and ranking systems where developers must inspect weights during debugging.

---

## Example Semantic Model

The intended optimization semantics are:

- `Bottom ⊕ α = α`
- `α ⊕ β = max(α, β)`
- `Bottom ⊗ α = Bottom`
- `α ⊗ Bottom = Bottom`
- `Ordinal(0) ⊗ α = α`
- `Ordinal(α) ⊗ Ordinal(β) = Ordinal(α + β)`

and valuation behaves like:

- `v(Bottom) = None`
- `v(0) = None` or a documented special case
- `v(ω^γ * c + lower) = γ`

The zero-ordinal valuation case should be documented explicitly during implementation.

---

## Recommended `0.21.0` Scope Boundary

### In scope

- ordinals below `ε₀`
- arena interning
- canonical CNF normalization
- comparison
- ordinal addition
- leading-exponent valuation
- semiring wrapper with explicit bottom
- formatting and examples

### Out of scope

- ordinals beyond `ε₀`
- multiplication / exponentiation as major public operations unless clearly needed
- natural/Hessenberg arithmetic as a parallel API surface
- symbolic proper-class ordinal claims
- highly generic theorem-framework abstractions around ordinals

---

## Open Design Preference Already Recommended

The main naming recommendation is:

- prefer `OrdinalId` publicly rather than a generic `NodeId`

This keeps the surface clear while still matching the intended implementation strategy of arena-interned nodes.

---

## Summary

The ordinal substrate should be implemented as a dedicated, arena-backed, below-`ε₀` CNF layer inside `amari-tropical`, with a separate bottom-extended optimization carrier for semiring use.

That gives a mathematically honest and practically useful substrate for ranking, valuation, and composition workloads without distorting the existing float max-plus layer.
