# CORE-W04 consumer impact & breaking-change rationale — revision 9

Companion to `2026-10-13-amari-core-w04-rotor-duality-contract.md` (revision
9 + table amendment). Context: amari-core has external consumers (33,326
downloads, 18,571 recent; dependents pinned `^0.24.1`); breaking changes are
acceptable but each must be either **mathematically forced** (an executed
counterexample leaves no sound alternative) or a **disclosed design choice**
with the viable alternative and its rejection rationale stated. This
revision separates those two classes explicitly and corrects the inventory,
migration recipes, and behavioral disclosures of revisions 1–2 (review
rounds R1b–R8; review loop converged at R8: 0 P1 / 0 P2 / 2 P3, both
applied — ready to accompany the contract to the operator).

## 0. Failure-mode guarantee — per-operation, compilation-honest

**What breaks compilation** — the concrete incompatible requirements
(R7-P2; adaptations such as `map_err(|_| fallback)` with a retained
`ExpError`, or methods available on both old and new types, may keep
code compiling and are not claimed to break):
- requiring the migrated expression itself to have its old **concrete
  type** (binding `Result<_, ExpError>` from `power`, `Option<Rotor>`
  from `from_vectors`, `Self`/`Multivector` from the value-returning
  methods, `()` from the setters);
- using **removed items** — the deleted generic
  `Vector::<P,Q,R>::hodge_dual`, removed projecting constructors, `mv`
  field literals;
- **trait obligations only the old type satisfies** — e.g. an enclosing
  `?` whose error type implements `From<ExpError>` but not
  `From<RotorError>` (R7-P2);
- `Option`-shaped control flow (`Some`/`None` arms, `Option`-boundary
  `?`) on `Option → Result` migrations.

Many consumers still compile through the migrations — `Err(_)` arms,
`is_ok()`, `.unwrap()`/`.expect()`, discarded results, and other
`Result`-generic patterns (R2–R6). **Therefore the behavioral
disclosures below are organized per operation and are independent of the
consumer's call pattern**: any compiling consumer of a listed operation
observes the listed delta. Consumers whose builds pass must re-audit
results of every §0b operation they call.

### §0b Behavioral deltas by operation (observable by any compiling caller)

1. **`compose` / `geometric_product` become raw products.** Today's public
   `compose` normalizes (with an identity fallback on failure). Contract
   §1.5 deletes both. Observed difference for the angle-1 Cl(1,1,0) boost
   composed with itself: today `[1.5430806348152437, 0, 0,
   −1.1752011936438014]`; raw product `[1.5430806348152444, 0, 0,
   −1.1752011936438018]`. Consumers needing today's normalized values must
   renormalize explicitly after composing.
2. **Component-index remapping behind compiling setter calls.**
   `set_bivector_component(k, v);` compiles unchanged (statement style
   ignores the new `bool`), but the blade it writes changes: in Cl(4,0,0),
   `set_bivector_component(2, 1.0)` writes `e23` today and `e14` under
   the contract's zero-based lexicographic map (R2-reproduced). **An
   ignored `false` return is the new silent-failure mode: consumers must
   handle the `bool`**, exactly the pattern the old silent-zero had.
   Same class: `Bivector::set_component` (additive, but shares the
   ignored-`bool` hazard).
3. **Fallback deletions inside already-`Result` constructors.**
   `from_bivector`/`from_multivector_bivector` already return `Result`;
   deleting the identity fallback changes run-time results (`Err` where
   `Ok(identity)` was returned) without any signature change. Consumers
   that relied on the fallback get an error instead of a silently wrong
   rotor.
3b-i. **`power`, `from_bivector`, `from_axis_angle` — error-type
   migration only (already `Result`)**: `Err(_)` arms, `is_ok()`,
   `.unwrap()`, `.expect()`, and discarded results compile unchanged
   (R3/R4/R6 — including typed success values with `Err(_)` fallbacks);
   boundary `?` compiles iff the enclosing error type gains/requires
   `From<RotorError>` (an enclosing `From<ExpError>`-only context
   breaks — R7-P2). Delta: baseline `power(1.0)` returns **identity**
   for the stored boost `[1.127625965206381, 0, 0, −0.5210953054937475]`
   (contract: the stored rotor **bit-exact**).
3b-ii. **`from_vectors`/`from_vectors_mv` — `Option → Result`**: the
   `Result`-generic patterns above are NOT symmetric here:
   `.unwrap()`/`.expect()` compile on both sides (R4-verified);
   `Some(x)`/`None` arms compile today and **break after** (E0308
   pattern-type mismatch); `Option`-boundary `?` compiles today and
   **breaks after** (E0277); `.is_ok()` does not compile today (E0599 on
   `Option`) and compiles after — a compile-*enabling* change, not a
   preserved pattern. Delta: baseline `from_vectors(&a, &a)` in Cl(0,2,0)
   returns `None` (an `.unwrap()` caller **panics today**) where the
   contract returns `Ok(identity)`.
3c. **`slerp` (`Self → Result`), `logarithm`, `apply`, `inverse`,
   `hodge_dual` (value → `Result`)**: typed consumers naming the old
   value types break; `Result`-generic and discarded patterns compile.
   Delta: **`slerp` in Cl(2,0,0) identity-to-identity panics today even
   when the result is discarded** (rotor.rs:168, index 4 of length 4);
   the contract returns `Ok(identity)`. Discarded calls of the others
   change from returned values to (discarded) `Err`s — benign at the
   call site; `apply`'s value-level delta is §0b-4's.
4. **`apply`/`apply_to_vector` divide by the certified reverse norm `N`**
   (compile-loud via the `Result` migration, C1 — listed here only for
   the behavioral delta): today the sandwich omits the division, so
   unnormalized stored rotors act scaled by `N`; exactly-normalized
   rotors are unchanged within rounding.

## 1. Typed, reproducible blast-radius inventory

Commands (from the repo root, against the merge base):

```sh
git grep -n '\.mv\b' <base> -- 'amari-*/**/*.rs' | grep -v ':amari-core/'
git grep -n 'hodge_dual(' <base> -- 'amari-*/**/*.rs' | grep -v ':amari-core/'
git grep -nE '\.(apply_to_vector|slerp)\(|rotor[^.]*\.apply\(|\.logarithm\(' <base> -- 'amari-*/**/*.rs' | grep -v ':amari-core/'
```

Results on `79a9653`, raw grep hits vs **type-classified migration-affecting
sites** (R2 reconciliation; the second column is what migrations actually
touch):

| Surface | Raw grep hits | Migration-affecting | Notes |
|---|---:|---:|---|
| `.mv` textual | 48 | **47 field-access sites (49 individual accesses — both automata lines carry two)** | network 36 (tests 20, examples 14, src 2), gpu tests 7, automata src 2, discovery 2 (src 1, tests 1); the 48th line is `0..mv.dimension()` at gpu/src/verification.rs:135 — a **local variable**, not a field access |
| `hodge_dual` | 9 | **9** | enumerative src (`geometric_algebra.rs`) |
| `Rotor::apply` (amari-core) | 5 regex | **2** | enumerative `transform` + relativistic `spacetime`; the 3 wasm hits are one `WasmGenericRotor` test and two `WasmRotor300` tests (`WasmRotor300` wraps `WasmGenericRotor`); the wrapper classes store `WasmGenericMultivector` and implement their own sandwich/inverse — no delegation to amari-core `Rotor` (verify at stage gates) |
| `Rotor::inverse` | — | **2** | `amari-relativistic/src/spacetime.rs:592` (`self.rotor.inverse()`) and `:723` (`composed.inverse()`); wasm inverse sites are multivector/independent-wrapper operations; reproducible: `git grep -nE '\.inverse\(\)' 79a9653 -- 'amari-relativistic/src' 'amari-wasm/src'` then classify receivers |
| `Multivector::logarithm` (rotor-relevant) | — | **0** | the revision-2 wasm claim was false; wasm sites named `logarithm` are dual-number documentation |
| `slerp`, `Rotor::reflect`, `Rotor::from_vectors` | 0 | **0** | within documented paths |
| **Unrelated names, excluded by type**: `ChowClass::power` (wasm), GF2 `from_vectors` (gpu), dual-number `logarithm` (amari-dual), term-substitution `apply` (rewrite ×31), gpu `apply_pipeline`/CA-rule `apply` | — | — | classified by receiver type, not name |

## 2. Per-change necessity, with alternatives

### C1. `inverse`/`apply`/`apply_to_vector` → `Result` — **mathematically forced**

- **Forcing counterexamples (two distinct fixtures, correctly separated per
  R1b-P2):**
  - *Exponential fixture* (angle-1 Cl(1,1,0) boost, stored scalar
    `1.127625965206381`, bivector `−0.5210953054937475`): its exact stored
    norm is `20282409603651673759639097286673/20282409603651670423947251286016
    ≠ 1`, so today's `inverse()` (the bare reverse) is **not** the stored
    element's inverse (correct values `[1.1276259652063807, 0, 0,
    0.5210953054937474]`).
  - *Dyadic representability fixture* (`R = 5/4 + (3/4)e12`, exact `N = 1` —
    its reverse **is** its inverse; this fixture is about the chain, not the
    inverse): **five** repeated squarings (not "32 squarings") via raw
    products store `R^32 = [2³¹, 2³¹]` with certified `N = 0` — finite
    coefficients, silently degenerate. Today's public `compose` would
    normalize (and fall back to identity on failure) — the raw-chain
    behavior is the W04 target, and it is why the checked operations must
    detect `N ≤ 0`.
- **Alternatives rejected:** silent non-finite output (wrong results);
  panics (runtime crashes); documented-domain-only (the domain depends on
  stored-value drift invisible to callers).
- **Migration:** mechanical `?`/`match`; errors `Degenerate` /
  `Composition` / `Exp(...)` carriers.

### C2. `logarithm`/`power`/`slerp`/`from_vectors` → `Result` — **mathematically forced**

- **Forcing counterexamples:** `power(1)` returns identity for Cl(1,1,0)
  boosts and **+1 for a stored −1 rotor**; `slerp` panics in Cl(2,0,0)
  and silently returns identity at `t=1` in Cl(4,0,0); `from_vectors`
  spurious-`None`s all negative-square signatures. None has a total sound
  infallible definition.
- **Migration:** `?`; `from_vectors` `None`-handling maps to named
  variants.

### C3. Private `mv` — **forced by type-level enforcement; chosen scope**

- **Forcing counterexample:** `from_multivector_bivector` returns a
  wrongly-graded `Ok` rotor from a plain vector (grade-1 coefficient
  −0.419) — invariants are unenforceable over a public mutable field
  (CORE-09/14).
- **Design choices within the forced direction** (alternatives + rejection
  rationale): none of the below change the privacy decision; they are the
  expressiveness guarantees retained:
  - Dimension-general construction stays available: `Vector::<5,0,0>` from
    coefficients is `Multivector` + `TryFrom` (contract §1.1 — **no
    construction dead-end exists**, R1b-P3 verdict; the §5 table now
    lists the escape APIs explicitly).
  - `Vector::from_multivector` / `Bivector::from_multivector` **removal**
    is a **disclosed design choice, not a forced consequence**: their
    baseline implementations silently project mixed-grade input to the
    wrapper grade. Retaining them as projecting constructors alongside
    checked `TryFrom` is mathematically sound; removal was chosen per the
    loudness doctrine (silent grade loss is the CORE-09 pattern) and
    operator decision D03.1. Migration for projection users:
    `Vector::try_from(mv.grade_projection(1))?` (explicit projection),
    likewise grade-2 for `Bivector`.
- **Migration:** reads `x.mv` → `x.as_multivector()` / `into_multivector()`;
  constructions → `Multivector` + `TryFrom`.

### C4. `reflect` semantics + `reflect_line` — **forced correction; migration corrected per R1b-P2**

- **Forcing counterexample:** docs promise hyperplane reflection;
  implementation performs line reflection (`reflect(e1,e1) = +e1`).
- **Migration (operand order matters — baseline is `reflect(v, n)`, new is
  `reflect_line(n, A)`):** old `reflect(&a, &n)` → **`reflect_line(&n,
  &a)?`**. Behavior-equivalence domain: **certified positive-square unit
  normals (`n² = +1`) only** — there `n⁻¹ = n` and the new formula matches
  baseline numerics. For `n² = −1` (contract-valid), `n⁻¹ = −n` exactly,
  so `reflect_line(&n, &a) = −n·a·n` differs from baseline `n·a·n` by a
  sign (R2-reproduced in Cl(0,1,0): coefficient −1 vs +1) — a
  **post-migration behavioral difference for explicitly migrated
  `reflect_line` calls** with negative-square normals (old `reflect`
  calls break at compile time on operand order and `Result`; this is not
  a compilation-preserving class). The migrated baseline unit test asserts
  `reflect_line(&e1, &e1) = +e1` with operands in the new order.
- **Consumers wanting documented hyperplane behavior:** `reflect(&n, &A)?`.

### C5. Dual split — **forced separation; compatibility wording corrected per R1b-P2**

- **Forcing counterexample:** Cl(2,0,1) documented formal complement
  `star(e3) = e12`; implementation returns zero (metric annihilation of
  null-generator blades). One name cannot denote both maps.
- **Compatibility scope (corrected):** the Euclidean 3D **mathematical
  basis table is preserved** (`⋆e1=e23`, `⋆e2=−e13`, `⋆e3=e12`, `⋆e12=e3`
  — basis inputs, contract-battery-tested); **existing calls still require
  `Result` migration** (the metric dual errs on degenerate pseudoscalars;
  typed duals are fallible). No general bit-exactness claim beyond the
  tested classes.
- **Migration:** 9 enumerative sites: degenerate-signature users decide
  which map they need (`formal_complement` infallible, metric
  `hodge_dual()?`); generic `Vector::<P,Q,R>::hodge_dual() -> Bivector`
  compile-errors — use `Multivector` methods or concrete 3D impls.

### C6. Component APIs — **dimension-correct indexing forced; `bool` return is a design choice**

- **Forcing counterexample:** `[3,5,6]` layout in every dimension —
  Cl(4,0,0) component 3 silently unwritten; smaller dims out of bounds.
- **Alternative (per R1b-P2):** repair the setter to dimension-correct
  behavior keeping `()` (panicking or ignoring out-of-range) plus an
  additive checked setter. **Rejected:** ignoring is the silent-zero
  pattern under correction; panicking contradicts the error-carrying
  surface; one checked setter is the minimal loud surface. This is policy
  (loudness doctrine), labeled as such — not mathematical necessity.
- **Migration:** `set_bivector_component(k, v)` → same name, `-> bool`;
  blade-index `Index`/`get` unchanged; `to_rotation_matrix`/axis-angle
  helpers move to concrete 3D impls — **generic callers compile-error**;
  3D consumers unchanged. `to_rotation_matrix` is now fallible,
  `Result<[[f64; 3]; 3], RotorError>`, propagating the certified-`N` action
  gate so a stored element with no certified action returns
  `Err(Degenerate)` instead of panicking (CORE-W04 R4-F18). Workflows
  needing matrices outside 3D have no replacement (honest limit,
  disclosed).
- **`e_ij`**: pure addition. **`Bivector::zero()`**: pure addition.
- **`e12`/`e13`/`e23`, `Vector::e1`/`e2`/`e3`, and the
  `Basis::e1`/`e2`/`e3`/`e12`/`e23`/`e31`/`e123` helpers (added R5-F20):** the
  named basis-blade constructors are now fallible
  (`Result<_, ComponentError>`) — they previously panicked on blades outside
  the ambient signature (`e12` needs `n >= 2`; `e13`/`e23`/`e3` need `n >= 3`;
  `e1`/`e2` need `n >= 1`/`n >= 2`). Valid-signature callers migrate with a
  justification `.expect("Cl(P,Q,R) has blade e…")`; generic callers handle
  `ComponentError::OutOfRange`.

### C7. `from_bivector`/`from_axis_angle` error-type migration — **forced failure-representation; new type is a design choice**

- **Forcing counterexample:** the constructors' silent
  `unwrap_or(identity)` fallback (§0b-3) must become errors — some error
  representation is forced.
- **Alternative (per R1b-P2):** add normalization/domain variants to
  `ExpError`, preserving the return types. **Rejected:** it spreads rotor
  construction concerns into the exponential error type and still breaks
  exhaustive matches; the dedicated `RotorError` with lossless
  `From<ExpError>` centralizes the construction surface (operator-endorsed
  §5 table). Labeled policy, not necessity.
- **Migration:** `Result<_, ExpError>` → `Result<_, RotorError>`;
  `ExpError`-matching adjusts to `RotorError::Exp(e)`.

## 3. Consumer-side limits (updated by R2-F11: rounded multi-plane power rejection disclosed)

Large-rapidity boosts (`Degenerate` = representability; parameterize by
generator+angle below threshold); `log(−1)`/multi-plane/non-Euclidean-2D
antipodal rejected with named errors (construct the plane explicitly).

**Integer/real powers of multi-plane composites (R2, restated per error
kind by R3-F16).** For an element whose exact reverse norm `R·R†` is
non-scalar — every rounded multi-plane composite, e.g.
`r12(0.3)∘r34(0.7)` built from simple rotors in orthogonal planes — the
power surface splits by exponent, not by a single rejection:

- `power(0)`/`power_integer(0)` return `Ok(identity)`;
- `power(1)`/`power_integer(1)` return `Ok` of a **bit-exact clone**;
- `power(−1)`/`power_integer(−1)` return `Err(Degenerate)` — the
  stored-element inverse's exact-`N` certificate fails;
- other integral exponents (e.g. `power(2)`) return `Err(Degenerate)` —
  the certified logarithm rejects with `Uncertified`, then the
  binary-composition fallback's per-boundary exact-`N` certificate fails;
- non-integral exponents (e.g. `power(0.5)`) return `Err(Uncertified)` —
  the logarithm rejection, a **different** error.

The surviving binary-composition fallback is defined by **exact
certification, not coefficient shape**: every finite `f64` coefficient is
itself dyadic, so "dyadic coefficients" is not the criterion — the
criterion is an exact `R·R†` that certifies **scalar and positive**, which
holds for simple rotors and exactly-certified multi-plane products (e.g.
disjoint-plane products whose dyadic arithmetic is exact), not for general
rounded composites. **Migration:** for multi-plane elements, compose the
simple factors/chain directly instead of calling `power_integer` on the
stored composite — this applies to the supported integer-power workflows;
the `{0, 1}` literal returns above remain available on the stored
composite itself.

## 4. Communication plan (unchanged; scope widened)

Single 1.0-line break (no partial 0.25.x breaks); CHANGELOG covers C1–C7
**and §0b behavioral changes** with the migrations above; crate-root
migration note post-merge.

## 5. What this document does not claim

External call-site usage beyond the workspace inventory (crates.io does
not expose it) — the §0b list plus compile-loudness bounds the risk to
build time plus the enumerated behavior deltas. Consumers relying on
silently-wrong behavior (C1's reverse-inverse, C4's line reflection under
the hyperplane name, C5's zeros, §0b's normalized compose) will see fixes
where they may have seen stability; the necessity arguments are the
executed counterexamples, and desirability judgments belong to the
operator.
