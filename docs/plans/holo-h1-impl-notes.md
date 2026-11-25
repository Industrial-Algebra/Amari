# H1 implementation — PR handoff

**Target:** develop, 0.25.0. The CPU and Rayon bind kernels and generated WGSL use `Cl3::PRODUCT_TABLE`. Full-word, grade and two-sided residual oracles test the arithmetic independently. WASM flat-key ingress validates the 32 factors before mutating memory. H3 memory-model and H2 numeric-policy changes are not part of this patch.

**Migration:** Raw `[1,e1,e2,e3,e12,e13,e23,I]` slots and their 32-block ABI are unchanged. Derived coefficients change: `e13 * e2` formerly produced `+I`; the correct result is `−I`. Stop readers of old computed bindings, allocate a new versioned cache namespace, regenerate derived keys from original operands, replay associations to rebuild traces/indexes, then switch readers. There is no lossless conversion from a compressed trace alone. Choose among `right_clifford_dual`, `hodge_dual` and `formal_complement` by semantics, not a blanket rename. JS clients must handle errors for singular/nonfinite keys; zero values are allowed. No claim of external `tracedecay` runtime coverage.

**Evidence (Rust 1.100.0, x86_64 Linux, four Cargo overrides unset):**

| Unit | Failing baseline → passing control |
|---|---|
| U1 | 1 arithmetic test failed (101) → 1 passed: 64 ordered pairs, both bracketings of 512 triples, direct-product blocks including Rayon. |
| U2 | 1 inverse test failed (101) → 2 passed (public inverse and frozen candidate certificate). Max unscaled coefficient errors in both products must be ≤1e−12. |
| U3 | 1 grade test failed (101) → 1 passed; all ordered homogeneous pairs and a mixed case. |
| U4 | Missing dual API (101) → 1 test passed; `cargo check -p amari-rewrite --features holographic-guidance` passed at the trait change. |
| U5 | Missing checked generator map (101) → 1 test passed; cyclic action, invalid maps, extreme shift, signed product preservation. |
| U6 | Device test failed on an actual wrong shader coefficient at batch 101 (101); Node failed mixed-key recall (1). Corrected shader passed on Mesa llvmpipe Vulkan: CPU dispatch 4 records/1024 coefficients, device readback 101 records/25856 coefficients, basis exact and mixed tolerance 1e−6. Built web bindings passed Node mixed/unit recall and rejection/atomicity checks. |

**Final gates:** holographic default and all features 169 passing each; rewrite `holographic-guidance` check and test (527 passing); native wasm/fusion tests 182 passing; scoped all-target/all-feature `clippy -- -D warnings`, workspace fmt and scoped docs passed. Fresh lock resolution, wasm generation (byte-identical surface), then structural regeneration, structural drift gate 1/1 and `catalog_wasm` 41/41. No rewrite files changed. The discovery holographic adapter consumes MAP, not the changed geometric dual API; fusion reexports compile. No new no-std claim (B1 remains deferred).

**CI:** `Holographic Shader Parity` provisions lavapipe, requires enumeration, then tests the selected Vulkan backend. The generic job's no-device skip is not evidence here. Operator: mark this job required in branch protection when it appears. CI and independent PR review follow handoff; nothing has been pushed.
