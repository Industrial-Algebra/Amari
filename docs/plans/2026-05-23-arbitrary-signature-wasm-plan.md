# Arbitrary-Signature WASM Multivector — Implementation Plan

> **REQUIRED SUB-SKILL:** Use the executing-plans skill to implement this plan task-by-task.

**Goal:** Replace per-signature concrete WASM types with a single `WasmGenericMultivector` that accepts runtime `(p, q, r)`, dispatches through a match table for DIM ≤ 6, and falls back to a generic Cayley-table path for larger signatures. Provide 8 fast-path aliases for common signatures.

**Architecture:**
1. `amari-core` gets two new public functions (`blade_product`, `generic_geometric_product`) — additive, no breakage.
2. `amari-wasm/build.rs` generates a match table for all 84 `(p, q, r)` triples with DIM ≤ 6.
3. `amari-wasm/src/generic.rs` holds `WasmGenericMultivector` and `WasmGenericRotor` — dispatch logic, Cayley-table fallback with caching.
4. `amari-wasm/src/lib.rs` replaces existing concrete types with fast-path aliases around `WasmGenericMultivector`.
5. `typescript/src/index.ts` gets a single `Multivector` class with `(p, q, r)` constructor and 8 factory methods.

**Tech Stack:** Rust (edition 2021), wasm-bindgen, TypeScript

**Design doc:** `docs/plans/2026-05-23-arbitrary-signature-wasm.md`

---

### Task 1: Add `blade_product` and `generic_geometric_product` to amari-core

**Files:**
- Create: `amari-core/src/generic.rs`
- Modify: `amari-core/src/lib.rs` (add `pub mod generic;` and re-exports)

**Step 1: Write `blade_product` function and tests**

Add to `amari-core/src/generic.rs`:

```rust
//! Generic (runtime-signature) Clifford algebra operations.
//!
//! These functions work directly on coefficient slices with a runtime
//! signature (p, q, r), unlike the const-generic Multivector<P,Q,R> API.
//! They serve the WASM fallback path and any consumer that needs
//! signature selection at runtime.

/// Compute the result basis-blade index and sign for the geometric product
/// of two basis blades `i` and `j` in Cl(p, q, r).
///
/// - `p`: number of basis vectors squaring to +1
/// - `q`: number of basis vectors squaring to -1
/// - `r`: number of basis vectors squaring to 0
/// - `dim`: total dimension = p + q + r
/// - `i`, `j`: basis-blade indices (0 .. 2^dim)
///
/// Returns `(result_blade_index, sign)` where sign is +1.0 or -1.0.
pub fn blade_product(p: usize, q: usize, r: usize, dim: usize,
                     i: usize, j: usize) -> (usize, f64) {
    let k = i ^ j; // XOR for result blade

    // Compute reordering sign: number of swaps to merge sorted bases
    let mut sign = 1.0f64;

    // For each bit set in j, count set bits in i to the right of it
    let mut remaining_i = i;
    let mut remaining_j = j;
    let mut swap_count = 0u32;

    for bit in 0..dim {
        if (j >> bit) & 1 == 1 {
            // Count set bits in i to the left (higher indices)
            let higher_bits = (remaining_i >> (bit + 1)).count_ones();
            swap_count += higher_bits;
        }
        remaining_i &= !(1 << bit);
        remaining_j &= !(1 << bit);
    }

    if swap_count % 2 == 1 {
        sign = -sign;
    }

    // Compute metric sign: for each bit set in BOTH i and j,
    // multiply by the metric of that basis vector
    let common = i & j;
    for bit in 0..dim {
        if (common >> bit) & 1 == 1 {
            if bit < p {
                // Positive signature: squares to +1, no sign change
            } else if bit < p + q {
                // Negative signature: squares to -1
                sign = -sign;
            } else {
                // Null signature: squares to 0 → product is zero
                return (k, 0.0);
            }
        }
    }

    (k, sign)
}

/// Compute the full geometric product of two multivectors in Cl(p, q, r).
///
/// `a` and `b` are coefficient slices of length 2^(p+q+r). Returns a new
/// coefficient vector of the same length.
pub fn generic_geometric_product(p: usize, q: usize, r: usize,
                                  a: &[f64], b: &[f64]) -> Vec<f64> {
    let dim = p + q + r;
    let basis_count = 1 << dim;
    let mut result = vec![0.0; basis_count];

    for i in 0..basis_count {
        let ai = a[i];
        if ai.abs() < f64::MIN_POSITIVE { continue; }

        for j in 0..basis_count {
            let bj = b[j];
            if bj.abs() < f64::MIN_POSITIVE { continue; }

            let (k, sign) = blade_product(p, q, r, dim, i, j);
            if sign != 0.0 {
                result[k] += sign * ai * bj;
            }
        }
    }

    result
}
```

**Step 2: Write tests in the same file**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    use crate::Multivector;

    #[test]
    fn test_blade_product_e1_e1_cl300() {
        // e1 * e1 = 1 in Cl(3,0,0)
        let (k, sign) = blade_product(3, 0, 0, 3, 1, 1);
        assert_eq!(k, 0);  // scalar
        assert!((sign - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_blade_product_e1_e2_cl300() {
        // e1 * e2 = e12 in Cl(3,0,0)
        let (k, sign) = blade_product(3, 0, 0, 3, 1, 2);
        assert_eq!(k, 3);  // e12
        assert!((sign - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_blade_product_e2_e1_cl300() {
        // e2 * e1 = -e12 in Cl(3,0,0)
        let (k, sign) = blade_product(3, 0, 0, 3, 2, 1);
        assert_eq!(k, 3);
        assert!((sign + 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_blade_product_e3_e3_cl210() {
        // e3 * e3 = -1 in Cl(2,1,0) — negative signature
        let (k, sign) = blade_product(2, 1, 0, 3, 4, 4);
        assert_eq!(k, 0);
        assert!((sign + 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_blade_product_null_signature() {
        // e1 * e1 = 0 when e1 is null (Cl(0,0,1))
        let (k, sign) = blade_product(0, 0, 1, 1, 1, 1);
        assert_eq!(k, 0);
        assert!((sign - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_generic_geometric_product_matches_const_generic_cl300() {
        let mv_a = Multivector::<3, 0, 0>::basis_vector(0);
        let mv_b = Multivector::<3, 0, 0>::basis_vector(1);
        let expected = mv_a.geometric_product(&mv_b);

        let a_coeffs: Vec<f64> = (0..8).map(|i| mv_a.get(i)).collect();
        let b_coeffs: Vec<f64> = (0..8).map(|i| mv_b.get(i)).collect();
        let result = generic_geometric_product(3, 0, 0, &a_coeffs, &b_coeffs);

        for i in 0..8 {
            assert!((result[i] - expected.get(i)).abs() < 1e-10,
                "Coefficient {} differs: {} vs {}", i, result[i], expected.get(i));
        }
    }

    #[test]
    fn test_generic_geometric_product_matches_const_generic_cl210() {
        // Cl(2,1,0): e3 squares to -1
        let mv_a = Multivector::<2, 1, 0>::basis_vector(2);
        let mv_b = Multivector::<2, 1, 0>::basis_vector(2);
        let expected = mv_a.geometric_product(&mv_b);

        let a_coeffs: Vec<f64> = (0..8).map(|i| mv_a.get(i)).collect();
        let b_coeffs: Vec<f64> = (0..8).map(|i| mv_b.get(i)).collect();
        let result = generic_geometric_product(2, 1, 0, &a_coeffs, &b_coeffs);

        for i in 0..8 {
            assert!((result[i] - expected.get(i)).abs() < 1e-10,
                "Coefficient {} differs: {} vs {}", i, result[i], expected.get(i));
        }
    }

    #[test]
    fn test_generic_geometric_product_random_cl410() {
        // Cl(4,1,0) — 32 coefficients, CGA signature
        let mut rng = fastrand::Rng::with_seed(42);
        let a_coeffs: Vec<f64> = (0..32).map(|_| rng.f64() * 2.0 - 1.0).collect();
        let b_coeffs: Vec<f64> = (0..32).map(|_| rng.f64() * 2.0 - 1.0).collect();

        let mv_a = Multivector::<4, 1, 0>::from_coefficients(a_coeffs.clone());
        let mv_b = Multivector::<4, 1, 0>::from_coefficients(b_coeffs.clone());
        let expected = mv_a.geometric_product(&mv_b);

        let result = generic_geometric_product(4, 1, 0, &a_coeffs, &b_coeffs);

        for i in 0..32 {
            assert!((result[i] - expected.get(i)).abs() < 1e-8,
                "Coefficient {} differs: {} vs {}", i, result[i], expected.get(i));
        }
    }
}
```

**Step 3: Register module in `amari-core/src/lib.rs`**

Add `pub mod generic;` to the module declarations and re-export:

```rust
pub mod generic;
pub use generic::{blade_product, generic_geometric_product};
```

**Step 4: Run tests**

```bash
cargo test -p amari-core -- generic
```

Expected: 8 tests pass.

**Step 5: Commit**

```bash
git add amari-core/src/generic.rs amari-core/src/lib.rs
git commit -m "feat(amari-core): add blade_product and generic_geometric_product for runtime-signature ops"
```

---

### Task 2: Build script for match-table generation

**Files:**
- Create: `amari-wasm/build.rs`
- Modify: `amari-wasm/Cargo.toml` (no changes needed, build.rs is auto-detected)

**Step 1: Write the build script**

```rust
//! Build script that generates the match-table dispatch arms for all
//! (p, q, r) signatures with DIM = p + q + r ≤ MAX_MATCH_DIM.

use std::env;
use std::fs;
use std::path::PathBuf;

const MAX_MATCH_DIM: usize = 6;

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let dest_path = out_dir.join("match_table.rs");

    let mut arms = String::new();

    for dim in 0..=MAX_MATCH_DIM {
        for p in 0..=dim {
            for q in 0..=(dim - p) {
                let r = dim - p - q;
                arms.push_str(&format!(
                    "            ({p}, {q}, {r}) => generic_product_impl::<{p}, {q}, {r}>(a, b, result),\n",
                    p = p, q = q, r = r
                ));
            }
        }
    }

    let count = arms.lines().count();
    let code = format!(
        r#"// Auto-generated by build.rs — DO NOT EDIT
// All (p, q, r) signatures where p+q+r ≤ {MAX_MATCH_DIM} ({count} combinations)

/// Dispatch to the correct Multivector<P, Q, R> monomorphization for a
/// given runtime signature (p, q, r).
///
/// `a` and `b` are coefficient slices of length 2^(p+q+r).
/// `result` is a mutable slice of the same length to write into.
#[inline]
pub(crate) fn dispatch_generic_product(
    p: usize,
    q: usize,
    r: usize,
    a: &[f64],
    b: &[f64],
    result: &mut [f64],
) {{
    match (p, q, r) {{
{arms}        _ => unreachable!("signature not in match table"),
    }}
}}

/// Helper: construct Multivector<P,Q,R>, compute geometric product, extract coefficients.
#[inline]
fn generic_product_impl<const P: usize, const Q: usize, const R: usize>(
    a: &[f64],
    b: &[f64],
    result: &mut [f64],
) {{
    let mv_a = amari_core::Multivector::<P, Q, R>::from_coefficients(a.to_vec());
    let mv_b = amari_core::Multivector::<P, Q, R>::from_coefficients(b.to_vec());
    let mv_c = mv_a.geometric_product(&mv_b);
    let basis_count = amari_core::Multivector::<P, Q, R>::BASIS_COUNT;
    for i in 0..basis_count {{
        result[i] = mv_c.get(i);
    }}
}}
"#,
        MAX_MATCH_DIM = MAX_MATCH_DIM,
        count = count,
        arms = arms,
    );

    fs::write(&dest_path, code).unwrap();
    println!("cargo:rerun-if-changed=build.rs");
}
```

**Step 2: Verify build script runs**

```bash
cargo build -p amari-wasm 2>&1 | head -5
```

Expected: Compiles with no errors.

**Step 3: Commit**

```bash
git add amari-wasm/build.rs
git commit -m "feat(amari-wasm): add build.rs to generate match-table dispatch for DIM ≤ 6"
```

---

### Task 3: Add `WasmGenericMultivector` and `WasmGenericRotor` to amari-wasm

**Files:**
- Create: `amari-wasm/src/generic.rs`
- Modify: `amari-wasm/src/lib.rs` (add `pub mod generic;`)

**Step 1: Write `generic.rs` — the core new module**

This module contains:
- `WasmGenericMultivector` — the single multivector type
- `WasmGenericRotor` — the single rotor type
- Cayley-table cache for fallback signatures
- All geometric algebra operations (geometric/inner/outer product, reverse, grade projection, normalize, inverse, exp, add, sub, scale)

Key design notes:
- The `geometric_product` method tries the match table first (via `include!` of the generated code), falls back to Cayley table
- The Cayley table is cached per `(p, q, r)` in a `thread_local! RefCell<HashMap<(usize,usize,usize), CayleyTable>>`
- `CayleyTable` stores a precomputed `Vec<(usize, f64)>` — one entry per `(i, j)` blade pair

The full implementation (see design doc for algorithm details):

```rust
use amari_core::{blade_product, generic_geometric_product, Bivector, Multivector, Rotor};
use std::cell::RefCell;
use std::collections::HashMap;
use wasm_bindgen::prelude::*;

// Include the build-script-generated match table
include!(concat!(env!("OUT_DIR"), "/match_table.rs"));

// ---- Cayley table cache ----

struct CayleyTable {
    /// For each (i, j) blade pair, precomputed (k, sign) or sentinel for unset.
    /// Stored as a flat array: table[i * basis_count + j] = Some((k, sign))
    data: Vec<Option<(usize, f64)>>,
    basis_count: usize,
}

impl CayleyTable {
    fn new(p: usize, q: usize, r: usize) -> Self {
        let dim = p + q + r;
        let basis_count = 1 << dim;
        let mut data = vec![None; basis_count * basis_count];

        for i in 0..basis_count {
            for j in 0..basis_count {
                let (k, sign) = blade_product(p, q, r, dim, i, j);
                data[i * basis_count + j] = Some((k, sign));
            }
        }

        Self { data, basis_count }
    }

    fn lookup(&self, i: usize, j: usize) -> (usize, f64) {
        self.data[i * self.basis_count + j].unwrap()
    }
}

thread_local! {
    static CAYLEY_CACHE: RefCell<HashMap<(usize, usize, usize), CayleyTable>> =
        RefCell::new(HashMap::new());
}

fn get_cayley_table(p: usize, q: usize, r: usize) -> std::cell::Ref<CayleyTable> {
    // We need to return a Ref that borrows from the HashMap.
    // To keep it simple, we ensure the entry exists, then call get again.
    // This unavoidably requires two lookups in the hot path, but the
    // HashMap hit is negligible compared to the product computation.
    CAYLEY_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        cache.entry((p, q, r)).or_insert_with(|| CayleyTable::new(p, q, r));
        drop(cache);
        CAYLEY_CACHE.with(|c| {
            // Reborrow as immutable
            let c = c.borrow();
            // Can't return a Ref to interior of HashMap directly,
            // so we clone.  The Cayley table is small relative to product cost.
            // For a cleaner implementation, store CayleyTable in an Rc.
            // For simplicity here, we just build on demand each call for the
            // generic fallback (it's the slow path anyway).
        })
    });
    // Actually, let's simplify: just compute on each call for the fallback.
    // The fallback is for DIM > 6 which is rare.  We document that the
    // first call is O(4^dim) and subsequent calls within the same product
    // chain reuse a locally-computed table.
    unimplemented!("see note above — final impl uses Rc<CayleyTable>")
}
```

**Update during implementation**: The Cayley cache will use `Rc<CayleyTable>` to avoid borrow issues. The table is computed once per `(p, q, r)` and shared across calls.

Full file will be written during implementation — this plan captures the structure.

**Step 2: Write tests**

Tests in `generic.rs`:
- `test_generic_multivector_cl300_basis_squares` — e1²=+1, e2²=+1, e3²=+1
- `test_generic_multivector_cl210_basis_squares` — e1²=+1, e2²=+1, e3²=−1
- `test_generic_multivector_cl310_basis_squares` — e0²=+1, e1²=+1, e2²=+1, e3²=−1 (Minkowski)
- `test_generic_multivector_cl030` — e1²=−1, e2²=−1, e3²=−1 (quaternion)
- `test_generic_multivector_outcome_class` — geometric product of random coeffs matches const-generic path for 10 random signatures with DIM ≤ 6
- `test_generic_multivector_fallback_dim7` — verify generic fallback works for DIM 7
- `test_generic_multivector_mismatched_signature` — geometric product of mismatched signatures returns error
- `test_generic_rotor_apply` — rotor composition works for Cl(3,0,0) and Cl(3,1,0)

**Step 3: Commit**

```bash
git add amari-wasm/src/generic.rs amari-wasm/src/lib.rs
git commit -m "feat(amari-wasm): add WasmGenericMultivector and WasmGenericRotor with match-table dispatch"
```

---

### Task 4: Replace concrete types in `lib.rs` with fast-path aliases

**Files:**
- Modify: `amari-wasm/src/lib.rs`

**Changes:**

1. Remove `wasm_multivector!` and `wasm_rotor!` macro invocations that generate `WasmMultivector` and `WasmSpacetimeMultivector`.
2. Replace with 8 fast-path alias structs: each is a thin wrapper around `WasmGenericMultivector` that pre-sets `(p, q, r)`.
3. Generate these via a `wasm_fastpath_alias!` macro that creates `#[wasm_bindgen]` structs with named factory methods and the full WASM API.
4. Remove standalone batch/perf ops for Cl(3,0,0) and Cl(2,1,0) — replace with generic `(p, q, r)` parameter versions.
5. Keep the module declarations, `init()`, and `console_log!` macro.

**Step 1: Write the new fast-path alias macro**

Each alias struct provides the same API as the current `WasmMultivector` but internally stores a `WasmGenericMultivector` with the correct `(p, q, r)` pre-set. The `geometric_product`, `inner_product`, etc. delegate to the generic struct.

**Step 2: Update BatchOperations and PerformanceOperations**

Add generic `batchGeometricProduct(p, q, r, a, b)` that constructs `WasmGenericMultivector` instances internally. Remove the old signature-specific methods (or keep as deprecated wrappers).

**Step 3: Update tests**

The existing `WasmMultivector` tests become tests of the fast-path alias. Add tests for the generic constructor path.

**Step 4: Commit**

```bash
git add amari-wasm/src/lib.rs
git commit -m "feat(amari-wasm): replace concrete types with fast-path aliases around WasmGenericMultivector"
```

---

### Task 5: Update TypeScript `index.ts`

**Files:**
- Modify: `typescript/src/index.ts`

**Changes:**

1. Import `WasmGenericMultivector` and `WasmGenericRotor` from the WASM package (in addition to the 8 fast-path alias structs).
2. Refactor `Multivector` class to accept optional `(p, q, r)` parameters. Default (`undefined`) = Cl(3,0,0).
3. Add 8 static factory methods (`euclidean3D()`, `spacetime2p1()`, `minkowski()`, `planar()`, `quaternion()`, `conformal()`, `euclidean5D()`, `split2D()`).
4. Each factory returns an `AlgebraHandle` (new type) that bundles the `(p, q, r)` triplet with named basis-vector accessors.
5. Remove the `SpacetimeAlgebra`, `SpacetimeRotor`, `SpacetimeBasisBlade`, `SpacetimeAlgebraBuilder` classes (merged into the generic `Multivector` and `Rotor` classes).
6. Keep `GA` and `ST` as convenience exports (aliases to the factories).
7. Add `MINK`, `PL`, `QUAT`, `CGA`, `P5D`, `S2D` exports.
8. Update `BatchOps` to accept `(p, q, r)` parameter.
9. Keep `BasisBlade` enum (still useful for Cl(3,0,0) fast path).

**Step 1: Write the refactored index.ts**

Key TypeScript interface:

```typescript
interface AlgebraHandle {
  readonly p: number;
  readonly q: number;
  readonly r: number;
  readonly dim: number;
  readonly basisCount: number;
  scalar(value: number): Multivector;
  zero(): Multivector;
  /** Named basis vectors */
  basisVector(index: number): Multivector;
  builder(): MultivectorBuilder;
}

class Multivector {
  constructor(p?: number, q?: number, r?: number);
  // ... all existing methods, aware of (p, q, r) ...
  static euclidean3D(): AlgebraHandle;    // Cl(3,0,0)
  static spacetime2p1(): AlgebraHandle;   // Cl(2,1,0)
  static minkowski(): AlgebraHandle;      // Cl(3,1,0)
  static planar(): AlgebraHandle;         // Cl(2,0,0)
  static quaternion(): AlgebraHandle;     // Cl(0,3,0)
  static conformal(): AlgebraHandle;      // Cl(4,1,0)
  static euclidean5D(): AlgebraHandle;    // Cl(5,0,0)
  static split2D(): AlgebraHandle;        // Cl(1,1,0)
}
```

**Step 2: Commit**

```bash
git add typescript/src/index.ts
git commit -m "feat(typescript): replace per-signature classes with generic Multivector + 8 fast-path factories"
```

---

### Task 6: Full test suite verification

**Step 1: Run all WASM tests**

```bash
cargo test -p amari-wasm
```

Expected: All tests pass (existing 116 + new generic tests).

**Step 2: Run amari-core tests**

```bash
cargo test -p amari-core
```

Expected: All existing tests pass (generic module is additive).

**Step 3: Verify full workspace builds**

```bash
cargo check --workspace
```

Expected: No errors.

**Step 4: Commit**

```bash
git commit -m "test: verify full test suite after arbitrary-signature refactor" --allow-empty
```

---

### Task 7: Final integration and documentation

**Step 1: Update CHANGELOG.md**

Add entry under `[0.23.0]`:

```markdown
### Added

- `WasmGenericMultivector` and `WasmGenericRotor` for runtime signature selection
- Match-table dispatch for all 84 DIM ≤ 6 signatures (compile-time optimized)
- Cayley-table fallback for DIM > 6 with lazy caching
- 8 fast-path aliases: Euclidean3D, Spacetime2p1, Minkowski, Planar, Quaternion, Conformal, Euclidean5D, Split2D
- `blade_product` and `generic_geometric_product` in `amari-core::generic`
- TypeScript `Multivector` class with `(p, q, r)` constructor and factory methods

### Changed

- Replaced per-signature `WasmMultivector`/`WasmSpacetimeMultivector` with `WasmGenericMultivector`
- `BatchOperations` and `PerformanceOperations` now accept generic `(p, q, r)` parameters
```

**Step 2: Commit**

```bash
git add CHANGELOG.md
git commit -m "docs: update changelog for arbitrary-signature WASM support"
```

**Step 3: Final verification and merge preparation**

```bash
cargo test -p amari-wasm -p amari-core
cargo check --workspace
git log --oneline
```
