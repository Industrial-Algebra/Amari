# Amari Project Context for Coding Assistants

This document reflects the current state of the repository as of **2026-03-27**.

## Project Overview

**Amari** is a Rust workspace centered on a broad mathematical computing platform.
It is no longer just a geometric/tropical/dual algebra project: the repository now spans a large family of domain crates plus integration layers for GPU, WebAssembly, TypeScript, and interactive examples.

### Current Version

- **Workspace version:** `0.19.1`
- **Primary crate:** `amari`
- **Rust edition:** `2021`
- **MSRV / rust-version:** `1.75`
- **Default toolchain file:** `nightly` (`rust-toolchain.toml`)
- **Practical note:** stable is still used for normal builds/tests in several places; nightly is primarily required for formal-verification workflows.

## What the Project Contains Now

### Workspace crates

The workspace currently includes these packages:

1. `amari` (umbrella crate)
2. `amari-core`
3. `amari-tropical`
4. `amari-dual`
5. `amari-network`
6. `amari-fusion`
7. `amari-info-geom`
8. `amari-automata`
9. `amari-enumerative`
10. `amari-relativistic`
11. `amari-gpu`
12. `amari-optimization`
13. `amari-flynn`
14. `amari-flynn-macros`
15. `amari-measure`
16. `amari-calculus`
17. `amari-holographic`
18. `amari-probabilistic`
19. `amari-functional`
20. `amari-topology`
21. `amari-dynamics`
22. `amari-wasm`

### Non-workspace project apps/packages

- `examples-suite/` — Vite + React interactive examples site
- `typescript/` — TypeScript wrapper/build flow for WASM output
- `examples/` — Rust and PureScript examples/documentation

## Current Mathematical / Product Scope

The current root README describes Amari as a unified platform covering:

- Geometric algebra / Clifford algebra
- Differential calculus
- Measure theory
- Probability theory on geometric spaces
- Functional analysis
- Algebraic topology
- Dynamical systems
- Vector symbolic architectures / holographic memory
- Optical field operations
- Relativistic physics
- Tropical algebra
- Automatic differentiation
- Fusion systems
- Information geometry
- Optimization
- Network analysis
- Cellular automata
- Enumerative geometry
- **GF(2) algebra** (newly highlighted in `0.19.1`)
- **Probabilistic contracts / verification** via `amari-flynn`

## Architecture

### Repository shape

The repo is organized around **domain crates** plus **integration crates**:

- **Domain crates:** `amari-core`, `amari-calculus`, `amari-measure`, `amari-functional`, `amari-topology`, `amari-dynamics`, etc.
- **Integration crates:**
  - `amari` — umbrella re-export crate
  - `amari-gpu` — GPU acceleration layer over domain crates
  - `amari-wasm` — WebAssembly bindings over domain crates

This dependency direction is documented explicitly in `amari-gpu/README.md`: integration crates consume domain crates, never the reverse.

### Umbrella crate behavior

`src/lib.rs` currently:

- always re-exports: `core`, `dual`, `tropical`, `network`, `fusion`, `info_geom`, `automata`, `enumerative`, `relativistic`
- conditionally re-exports via features:
  - `measure`
  - `calculus`
  - `holographic`
  - `probabilistic`
  - `functional`
  - `topology`
  - `dynamics`
  - `flynn`
  - `gpu`
  - `optimization`
- defines a unified `AmariError`
- has an essentially placeholder `src/main.rs` (`Hello, world!`)

### Feature model

The root crate has a very small default surface:

- `default = []`
- richer functionality is opt-in through feature flags
- `full` enables all optional crates
- `deterministic` is a dedicated feature for networked physics / bit-exact behavior

## Current State vs Older Context

The older consolidated context in this folder is outdated in several major ways:

- it still describes the project as **v0.9.6**
- it focuses primarily on the multi-GPU milestone
- it omits many now-present crates (`measure`, `calculus`, `holographic`, `probabilistic`, `functional`, `topology`, `dynamics`, `optimization`, `flynn`, `flynn-macros`, `wasm`)
- it understates the breadth of the examples and frontend packages
- it does not mention the current `0.19.1` emphasis on **GF(2)** and **probabilistic contracts**

## Current GPU State

`amari-gpu` is still important, but it should no longer be treated as the sole center of the repository.

### Implemented integrations per README

`amari-gpu` currently claims acceleration for:

- core geometric algebra
- information geometry
- relativistic operations
- network analysis
- measure theory
- calculus
- dual numbers
- enumerative geometry
- automata
- fusion
- holographic memory / optical field ops
- probabilistic computations
- functional analysis
- topology
- dynamics

### Current limitation

Both `docs/1.0-audit.md` and `amari-gpu/README.md` make clear that GPU support still has an important caveat:

- the crate needs validation on real modern GPU hardware
- several tests are environment-sensitive
- some WGSL shaders use workaround-heavy code paths
- tropical GPU integration is still disabled

This is still an active risk area before a 1.0 release.

## Testing / Validation Snapshot

### What was verified during repo inspection

- `cargo +stable test --workspace --all-features --quiet` successfully ran through a large portion of the workspace and many crates passed hundreds of tests.
- That run **timed out** in `amari-gpu` after several long-running GPU-related tests had been running for over 60 seconds in the current environment.
- A plain `cargo test -q` from the root completed only a small subset and also triggered rustup toolchain syncing because the repo defaults to nightly.

### Practical interpretation

- **Most non-GPU crates appear healthy from a testing perspective in this environment.**
- **GPU-heavy validation remains environment-dependent and is not fully confirmable here without appropriate hardware/runtime support.**

### Current audit document

`docs/1.0-audit.md` is the most important state-of-the-project document for correctness work.
It records:

- `amari-core` audit work as complete/pending manual review
- `amari-enumerative` GF(2) extension audit work as complete/pending manual review
- the remaining crates as still pending in the staged 1.0 audit sequence
- explicit GPU limitations and pre-1.0 recommendations

## Notable Recent State Changes

### 1.0 readiness effort is active

The repository is clearly in a **pre-1.0 hardening phase**, not an early exploratory phase anymore.
Evidence:

- `docs/1.0-audit.md`
- `docs/roadmap/V1_READINESS_PLAN.md`
- many per-crate READMEs with mature API sections
- release/version scripts across the workspace

### GF(2) work is now first-class

The audit doc describes a newly added `gf2` module in `amari-core` and downstream `amari-enumerative` extensions, including:

- GF(2) scalars, vectors, matrices
- binary Clifford algebra / binary multivectors
- binary Grassmannian and representability utilities
- coding-theory support
- Kazhdan-Lusztig related functionality in enumerative geometry

### Flynn/probabilistic contracts are now part of the platform

`amari-flynn` and `amari-flynn-macros` are now part of the workspace and the root README explicitly highlights:

- SMT-LIB2 proof obligation generation
- Monte Carlo verification
- probabilistic value tracking
- rare event classification

### Frontend/examples surface is larger

There is now a substantial examples/application layer:

- `examples-suite/` is a modern React/Vite app at version `0.19.1`
- Netlify deployment config exists (`netlify.toml`)
- `amari-wasm` publishes npm package `@justinelliottcobb/amari-wasm` version `0.19.1`
- `examples/` includes Rust and PureScript materials, plus learning-path and documentation files

## Current Documentation Signals

### Most useful current docs

For understanding actual current state, prioritize:

1. `README.md` — current top-level scope and positioning
2. `Cargo.toml` — authoritative workspace members/features/version
3. `docs/1.0-audit.md` — best correctness/status snapshot
4. `docs/roadmap/V1_READINESS_PLAN.md` — current direction toward 1.0
5. crate-level READMEs — per-domain current APIs and feature claims
6. `amari-gpu/README.md` — GPU integration architecture and limitations

### Historical docs still present

There are many archive/roadmap docs referring to older versions (`0.8.x` through `0.9.x`).
These are useful for historical context, but should not be treated as authoritative project state.

## Development Conventions That Still Fit

The older context's general engineering guidance is still broadly correct:

- preserve mathematical invariants
- prefer explicit error handling
- use type-level encodings and phantom types where established
- avoid unsafe code unless justified
- keep performance in mind
- add tests for mathematical properties and regressions

But coding assistants should also recognize that:

- the workspace is now broad and unevenly matured across crates
- some README claims are ahead of what can be verified locally without special hardware
- `docs/1.0-audit.md` is the best indicator of what has actually been audited for correctness

## Current Project Structure Snapshot

```text
amari/
├── Cargo.toml
├── src/                        # umbrella crate
├── amari-core/
├── amari-tropical/
├── amari-dual/
├── amari-network/
├── amari-fusion/
├── amari-info-geom/
├── amari-automata/
├── amari-enumerative/
├── amari-relativistic/
├── amari-gpu/
├── amari-optimization/
├── amari-flynn/
├── amari-flynn-macros/
├── amari-measure/
├── amari-calculus/
├── amari-holographic/
├── amari-probabilistic/
├── amari-functional/
├── amari-topology/
├── amari-dynamics/
├── amari-wasm/
├── examples/
├── examples-suite/
├── typescript/
├── docs/
├── benches/
└── tests/
```

## Practical Assistant Guidance

### When working in this repo

- Treat this as a **large Rust workspace**, not a single crate.
- Check whether a capability belongs in a domain crate, `amari-gpu`, or `amari-wasm` before editing.
- Verify feature flags before assuming modules are available by default.
- Use `cargo +stable ...` when nightly sync/toolchain issues get in the way of ordinary validation.
- For correctness/risk questions, consult `docs/1.0-audit.md` first.
- Be cautious about GPU assumptions unless you have confirmed hardware-backed execution.

### Good default commands

```bash
# Workspace tests on stable
cargo +stable test --workspace --all-features

# Basic root package test
cargo test

# Format
cargo fmt --all

# Lint
cargo clippy --workspace --all-features -- -D warnings

# Build docs
cargo doc --workspace --no-deps
```

## Bottom Line

**Amari is currently a broad `0.19.1` mathematical computing workspace approaching 1.0, with substantial domain coverage, active audit/readiness work, strong non-GPU test coverage, and unresolved GPU validation risk.**
