# amari-rewrite

Foundational abstract and term rewriting systems for the Amari library.

`amari-rewrite` is a toolkit for building and exploring rewrite systems over
Amari-owned or user-owned data structures. The 0.23.0 target is a stable
symbolic core plus explicitly feature-gated experimental research surfaces.

## Stability tiers

Stable/default surface:

- `rewritable`: path/subterm traversal for user-owned data
- `ars`: abstract rewrite systems over arbitrary rewritable values
- `trs`: first-order terms, variables, substitutions, matching, and rewrite rules
- `inverse`: bounded predecessor/backward search
- `synthesis`: anti-unification and basic rule inference from examples

Experimental feature-gated surfaces:

- `macros`: future derive/helper macro ergonomics
- `smt`: solver-backed validation and synthesis interfaces
- `neural`: differentiable/neural rewrite trait scaffolding
- `network`: optional `amari-network` bridge for rewrite-search guidance

The default crate intentionally avoids external e-graph, SMT, neural, and tensor
framework dependencies.
