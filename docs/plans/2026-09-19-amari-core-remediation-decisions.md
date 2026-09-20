# amari-core Mathematical Contract Decisions — Implementation Plan Register

> **REQUIRED SUB-SKILL:** Use the executing-plans skill only after the dependent decisions and package plan are approved. A pending option below is not an implementation instruction.

**Goal:** Resolve mathematical domains, numerical semantics and invariant-bearing APIs explicitly before implementing consequential core changes.

**Architecture:** Each decision states the defect it addresses, alternatives, evidence required and dependent work packages. Record the maintainer's selected contract and rationale; do not turn a tentative recommendation into an approved design.

**Tech Stack:** Existing Rust const generics and core types, independent algebraic/numerical references, compile-fail tests, Cargo feature/toolchain matrices. No dependency, solver or verification-tool selection is implicit.

---

[Programme](2026-09-19-amari-1.0-audit-remediation-programme.md) · [Work packages](2026-09-19-amari-core-remediation-work-packages.md)

## CORE-D00 — Governing compatibility rule — APPROVED

**Maintainer decision, 2026-09-19:** mathematical correctness is paramount. Breaking changes are completely acceptable; consumers should expect correct mathematics and will be updated accordingly. Breakage can reveal earlier bugs or mistakes and is useful evidence to investigate.

Consequences:

- Do not preserve an unsound signature, invalid domain promise or defective result for compatibility.
- Do not default to deprecation layers or duplicate APIs merely to retain old call sites.
- Update consumers to the corrected contract, including their mathematical assumptions—not just their syntax.
- Remaining approval gates concern **which mathematical/domain/numerical design is correct and coherent**, not whether breakage is permissible.
- Coordinate versioning/release placement and independently review changes; this does not authorize a release merge, tag or publication.

## Pending decision overview

| ID | Decision | Blocks | State / owner |
|---|---|---|---|
| CORE-D01 | Metric, coefficient error, equality, zero and normalization contracts | W02; numerical policy for W03/W04/W07 | Pending / unassigned |
| CORE-D02 | General inverse/exponential domains, algorithms, error/resource bounds | W03/W04 | Pending / unassigned |
| CORE-D03 | Rotor domains, interpolation and logarithm branch policy | W07a/b/c; constructor domains in W06 | Pending / unassigned |
| CORE-D04 | Duality and reflection conventions, names and domains | W05/W07d; typed dual in W06 | Pending / unassigned |
| CORE-D05 | Grade/dimension/rotor invariant representation | W06 | Pending; awaits phantom audit / unassigned |
| CORE-D06 | Table/representation/resource strategy | W08 | Pending; awaits corrected benchmarks and unsafe audit / unassigned |
| CORE-D07 | Supported feature/toolchain/target verification matrix | W00 and all merge/closure gates | Pending / unassigned |

Each row is a discussion umbrella, not a bundled multi-question approval. Resolve independent API or algorithm choices in separate focused decisions/subrecords when reached. Research can proceed now; implementation of a blocked choice cannot.

## CORE-D01 — Separate mathematical quantities from error measures

**Evidence:** CORE-05/07; indefinite `norm()` masks nonzero null elements, drives incorrect exponential dispatch and falsely establishes approximate equality. The current docs also overstate general-multivector norm properties.

**Choices to resolve:**

1. Whether existing ambiguous `norm`/`magnitude` names retain an explicitly metric meaning or are replaced with names that distinguish metric quadratic form, coefficient norm and domain-specific magnitude. Renaming is acceptable; choose clarity, not compatibility.
2. Coefficientwise absolute/relative approximate-equality semantics, supported epsilon inputs and treatment of non-finite coefficients. Do not silently choose a tolerance from a failing example.
3. Exact identity checks versus explicit approximate pruning; include the `num_traits::Zero` implementation, not just inherent methods.
4. Supported normalization domains and errors: a vector may have negative or null square; a general multivector's scalar norm does not prove versor membership.

**Design alternatives:** distinct named operations/types with constrained normalization; or a smaller general API plus checked domain-specific wrappers. Either must keep signed metric information available and must not let indefinite length stand in for coefficient error.

**Required evidence:** null differences, positive/negative vectors, general mixed grades, very small/large finite coefficients, non-finite policy tests, and type-resolved caller inventory for comparisons/convergence/normalization. Demonstrate mathematical properties before stating them in docs.

**Not blocked by D01:** W01 can remove approximate pruning from foundational arithmetic using private structural-zero checks without deciding the public comparison/normalization API.

## CORE-D02 — General inverse and exponential numerical contracts

**Evidence:** CORE-01/03/04/05/06. A scalar-square fast path is not a general algorithm; fixed truncation is not convergence.

**Inverse alternatives to evaluate:**

- Checked blade/versor fast paths plus a general finite-dimensional linear solve using the left/right multiplication representation, with explicit cost/conditioning limits.
- Algebra-specific constructive inversion where its dimensional/signature coverage is established, with a correct general fallback or enforced unsupported-domain errors.

A plan must justify the algorithm and resource growth rather than selecting one solely because it is easy to implement. `None` for all mixed-grade inputs is not an acceptable surrogate for general inversion.

**Exponential alternatives to evaluate:**

- Scalar-square/nilpotent special cases plus a controlled scaling-and-series or rational approximation for the general case, with proved/validated bounds and any required inverse dependency made explicit.
- A supported decomposition into commuting generators, with demonstrated completeness for the advertised domains and an explicit fallback/restriction elsewhere.

Do not claim a decomposition valid for arbitrary indefinite/degenerate algebras without establishing its preconditions.

**Contracts to decide separately:**

- Supported dimensions/signatures/grades; caller-visible resource caps.
- Distinct outcomes for singular, unsupported, ill-conditioned, non-finite and non-convergent inputs.
- Absolute/relative residual and convergence criteria; public control of tolerances/iteration budgets if needed.
- Whether/how to extend `CoreError`/`CoreResult`. Inspect both current std/no_std definitions; an error variant must not silently exist in only one feature configuration.
- Any new dependency and its numerical, licensing, MSRV, no_std and maintenance implications.

**Required references/tests:** exact inverse `(2-e1)/3`, zero divisor `1+e1`, mixed-grade and degenerate-signature examples, two-sided residuals, scalar exponentials, `exp(N)=1+N` for nonzero square-zero N, commuting plane factorization, and an independent high-precision/reference method for nontrivial random cases. Do not use the algorithm under test as its own oracle.

**Decision output:** one approved domain/algorithm/error record per operation, with cost bounds and a detailed ≤8-task implementation subplan. Choosing inversion does not automatically choose the exponential or vice versa.

## CORE-D03 — Rotor-group capabilities and branches

**Evidence:** CORE-10/11/12; valid construction already creates non-Euclidean rotors, while interpolation/logarithm/vector mapping assume Euclidean quaternion behavior.

**Alternatives:**

- A genuinely supported general rotor abstraction with appropriately domain-specific interpolation, logarithm and mapping operations.
- Explicitly separated/limited rotor capabilities (e.g. Euclidean 3D interpolation versus supported general-signature action), rejecting unavailable operations through types or precise errors.

**Independent decisions needed:**

- Spin-group versus vector-rotation equivalence: when are R and −R distinguishable? Preserve the exposed rotor element for algebraic laws such as `R^1=R`.
- A logarithm branch policy at −1 and for multi-plane rotations. An ambiguous logarithm is not the zero logarithm.
- Interpolation domains and shortest-path policy; changing loop bounds does not prove group membership.
- Circular, hyperbolic and parabolic cases; same/antipodal/null/incompatible causal-class vector mappings.
- Definition of allowed `from_vectors` behavior for vectors of unequal metric magnitude: direction mapping versus an impossible isometry of their full magnitudes.

**Required evidence:** all original CORE-10/11/12 reproductions, independent group-law checks, full `R reverse(R)=1`, preservation of supported grades/metric products, tiny resolvable angles, branch endpoints and approved domain errors. Name domain exclusions explicitly; migrate consumers that previously relied on accidental availability.

## CORE-D04 — Name and implement distinct geometric operations

**Evidence:** CORE-08/09/13. Degenerate Hodge prose and implementation disagree; hyperplane reflection prose describes a different operation from the existing unit test.

**Duality decision:** select explicit orientation/sign conventions and decide whether to expose metric Hodge, a metric-induced degenerate map, metric-free complement, or separate named operations. Define the domain and grade of each result. Do not pretend the null pseudoscalar has an inverse.

**Reflection decision:** distinguish hyperplane reflection from reflection about a line. Decide supported signatures, normal validation and vector-only versus general-multivector outermorphism semantics. In Euclidean space a hyperplane reflection negates the normal and preserves tangents; a line reflection does the opposite on those vectors. A global minus applied to every grade is not automatically the general-multivector extension.

**Required evidence:** basis/coefficient-level orientation tables, nondegenerate double-dual signs, degenerate/null examples, reflection normal/tangent action and involution, arbitrary-grade examples where promised, plus caller interpretation inventory. Broken old tests must be replaced with the correct approved expectations, not used to select a false convention.

**Output:** separate focused duality and reflection records. Their names and tested contracts must agree before the corresponding findings close.

## CORE-D05 — Enforce actual invariants, not decorative phantom types

**Prerequisite:** finish the chronological audit of `unicode_ops.rs`, `verified.rs`, `verified_contracts.rs`, `verified_laws.rs` and their associated tests. Establish what is reachable, enforced and actually proved.

**Evidence:** CORE-09/14/15 and TYPE/API-01. Public `mv` fields bypass grades; a valid method constructs a grade-three `Bivector`; raw constructors and 3D-only helpers are available more broadly than their implementation.

**Alternatives to assess against existing types:**

- Private grade-constrained wrappers with checked construction and dimension-specific capabilities.
- A unified grade-indexed representation reusing suitable existing verified types, with explicit general multivectors where compile-time grade expressions are impractical.

Choose the smallest coherent representation that genuinely enforces the contract. Do not introduce a second incompatible hierarchy before evaluating the first; do not force compile-time proofs beyond what the supported Rust toolchain can express soundly.

**Specific boundaries:**

- Construction, mutation, conversion, deserialization and raw coefficient interop.
- Validation versus projection: discarding unwanted grades is not proof the input had the promised grade.
- Unit rotor membership versus scalar magnitude normalization; even-grade alone is insufficient.
- Dimension-specific methods, component/blade indexing and checked resource bounds.
- General float/precision backend constraints and Send/Sync/variance implications of any new marker types.

**Required evidence:** compile-fail cases for every promised static exclusion, runtime error tests for dynamic input, all public ingress-path inventory and downstream migration build/tests. Select compile-fail tooling deliberately; adding a new test dependency is not automatic.

**Output:** approved public API/representation design with exact signatures and migration inventory. That concrete design, not compatibility with `pub mv`, governs implementation.

## CORE-D06 — Performance within a correctness/resource envelope

**Evidence:** PERF-01/PERF/API-02. A full 4^n Cayley table is rebuilt for each product; struct alignment does not establish coefficient-buffer alignment.

**Alternatives:** caller-owned reusable tables/contexts, bounded shared immutable caches, or on-demand blade multiplication with measured dimension/density-dependent choices. Include simpler no-cache designs in the comparison; a global cache is not a foregone conclusion.

**Decide:** supported dimension/resource budgets, concurrency and no_std ownership, cache eviction/lifetime where applicable, accumulation ordering/determinism obligations, and whether representation/alignment changes are justified.

**Required evidence:** corrected-kernel benchmarks with consumed outputs, per-operation allocation and peak-memory evidence, repeated/changing signature workloads, dense/sparse regimes, target/toolchain details, and the independent arithmetic oracle. Complete the allocation/SIMD unsafe audit before accepting alignment-based optimization claims.

## CORE-D07 — Verification matrix and scope boundaries

**Evidence:** the default suite passes but no-default compilation, std-only test compilation and default warning-denied Clippy fail. Existing CI also covers configurations outside the four reviewed source files.

**Proposed baseline rows for W00/W01, to approve before implementation:**

| Row | Commands / purpose |
|---|---|
| Default | `cargo test -p amari-core`; `cargo clippy -p amari-core --all-targets -- -D warnings`; `RUSTDOCFLAGS='-D warnings' cargo doc -p amari-core --no-deps` |
| Minimal std | Same scoped test/Clippy checks with `--no-default-features --features std`; verify disabled GF(2)/phantom tests are explicitly gated |
| No default library | `cargo check -p amari-core --no-default-features --lib`; warning-denied scoped Clippy; this is a host no_std library check, not target execution proof |
| Formatting | `cargo fmt -p amari-core --check`; full required repository formatting check before PR completion |
| Independent regressions | Promoted production oracle plus finding-specific tests in debug and release |

For actual execution record an approved exact Rust toolchain and dependency resolution; use `--locked` after explicitly establishing the initially ignored lock. `--offline` is an environment choice, not a correctness exclusion.

**Expand/allocate separately:** `phantom-types`, `gf2`, `high-precision`, `native-precision`, `serialize`, `parallel`, relevant combinations/aliases, supported MSRV/stable/nightly, target execution, and formal verification. The `wasm-precision` feature is a pure-Rust alias: excluding the WASM binding crate does not justify excluding its CPU-testable precision behavior.

Formal-contract compilation, execution of assertions, and successful external proof runs are different evidence. Establish the actual Creusot tooling/version and reachable claims before promising a proof gate.

**Non-negotiable scope rule:** these scoped local rows do not replace current required PR CI or the release matrix. GPU/WASM audit exclusions do not authorize disabling their integration checks. If an excluded backend blocks an otherwise necessary core change, coordinate its owner/update and record the blocker; do not weaken the gate silently.

**Decision output:** exact supported matrix, owner for each extended row, hardware/tooling needs, and explicit time-bounded exceptions where the maintainer approves them. Never infer an exception from an inconvenient failure.

## Decision record template

```text
Decision ID / subdecision:
Question and affected finding IDs:
Baseline and evidence:
Options and mathematical preconditions:
Maintainer's explicit selected contract:
Rationale / numerical and resource bounds:
Exact API signatures or separately approved design reference:
Consumer changes required:
Acceptance/reference/compile-fail tests:
New dependencies or tooling, if any:
Owner / approval date / release coordination:
Remaining unresolved questions:
```

Only CORE-D00 is approved in this checkpoint. Pending rows must not be reported as settled architecture.
