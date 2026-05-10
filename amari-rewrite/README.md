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


## Quick start: TRS simplification

```rust
use amari_rewrite::{trs::{Rule, Term, TermSystem}, RewriteResult};

fn main() -> RewriteResult<()> {
    let system = TermSystem::new(vec![
        Rule::new(
            Term::sym("add", [Term::constant("0"), Term::var("X")]),
            Term::var("X"),
        )?,
    ]);

    let term = Term::sym("add", [Term::constant("0"), Term::constant("a")]);
    assert_eq!(system.normalize_with_limit(&term, 4)?, Term::constant("a"));
    Ok(())
}
```

## Examples

- `symbolic_simplification.rs`: implement `Rewritable` for a user-owned expression enum.
- `peano_trs.rs`: normalize Peano-style terms with checked TRS rules.
- `inverse_search.rs`: enumerate bounded predecessor terms.
- `infer_rule_from_examples.rs`: infer a basic rewrite rule from positive examples.

Run one with:

```bash
cargo run -p amari-rewrite --example peano_trs
```
