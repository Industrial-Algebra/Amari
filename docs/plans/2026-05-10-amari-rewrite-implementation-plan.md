# amari-rewrite Implementation Plan

> **Status:** Implemented for 0.23.0. The comprehensive 0.25 rewrite/inverse continuation is `2026-07-24-amari-rewrite-inverse-expansion-implementation-plan.md`.
>
> **REQUIRED SUB-SKILL:** Use the executing-plans skill to implement this plan task-by-task.

**Goal:** Add the `amari-rewrite` workspace crate for Amari 0.23.0 with a stable ARS/TRS/inverse/synthesis core and feature-gated experimental neural/SMT/network scaffolding.

**Architecture:** `amari-rewrite` is a standalone foundational crate. The default build exposes `rewritable`, `ars`, `trs`, `inverse`, `synthesis`, and `prelude`. Experimental `neural`, `smt`, `macros`, and `network` modules are feature-gated; `network` depends optionally on `amari-network` and builds on `neural`.

**Tech Stack:** Rust 2021, Cargo workspace versioning, `thiserror`, optional `serde`, optional `amari-network`, test-first implementation with `cargo +stable test -p amari-rewrite`.

---

## Execution rules

- Use TDD for behavior: write each test first, run it and confirm RED, implement minimal code, run GREEN.
- Commit after each task or small group of tightly related files.
- Do not add external rewrite, SMT, neural, or tensor dependencies in default features.
- Keep public docs honest: neural/SMT/network modules are experimental scaffolding in 0.23.0.

---

### Task 1: Scaffold workspace crate

**Files:**
- Modify: `Cargo.toml`
- Create: `amari-rewrite/Cargo.toml`
- Create: `amari-rewrite/src/lib.rs`
- Create: `amari-rewrite/src/prelude.rs`
- Create: `amari-rewrite/README.md`

**Step 1: Add workspace membership and dependency wiring**

Modify root `Cargo.toml`:

- Add `amari-rewrite` to `[workspace].members`.
- Add workspace dependency:

```toml
amari-rewrite = { path = "amari-rewrite", version = "0.22.0" }
```

Note: if the active branch has already bumped to 0.23.0, use `0.23.0` consistently instead.

**Step 2: Create crate manifest**

Create `amari-rewrite/Cargo.toml`:

```toml
[package]
name = "amari-rewrite"
version.workspace = true
authors.workspace = true
edition.workspace = true
license.workspace = true
rust-version.workspace = true
description = "Abstract and term rewriting systems for the Amari library"
repository = "https://github.com/justinelliottcobb/Amari"
homepage = "https://github.com/justinelliottcobb/Amari"
keywords = ["mathematics", "rewriting", "trs", "rules", "synthesis"]
categories = ["mathematics", "science", "algorithms"]

[dependencies]
thiserror = { workspace = true }
serde = { workspace = true, optional = true }
amari-network = { workspace = true, optional = true }

[features]
default = ["std"]
std = []
serialize = ["dep:serde"]
macros = []
smt = []
neural = []
network = ["dep:amari-network", "neural"]
```

**Step 3: Create minimal lib and prelude**

Create `amari-rewrite/src/lib.rs`:

```rust
//! Abstract and term rewriting systems for Amari.
//!
//! `amari-rewrite` provides foundational rewriting tools: abstract rewriting
//! systems (ARS), first-order term rewriting systems (TRS), bounded inverse
//! rewriting, and lightweight rule synthesis via anti-unification.

#![cfg_attr(not(feature = "std"), no_std)]

pub mod prelude;
```

Create `amari-rewrite/src/prelude.rs`:

```rust
//! Common imports for `amari-rewrite`.
```

Create a README with the same stability-tier message from `docs/plans/2026-05-10-amari-rewrite-design.md`.

**Step 4: Verify scaffold**

Run:

```bash
cargo +stable check -p amari-rewrite
```

Expected: PASS.

**Step 5: Commit**

```bash
git add Cargo.toml amari-rewrite
git commit -m "feat: scaffold amari-rewrite crate"
```

---

### Task 2: Add error, path, and Rewritable core

**Files:**
- Create: `amari-rewrite/src/error.rs`
- Create: `amari-rewrite/src/rewritable.rs`
- Modify: `amari-rewrite/src/lib.rs`
- Modify: `amari-rewrite/src/prelude.rs`
- Create: `amari-rewrite/tests/rewritable_expr.rs`

**Step 1: Write failing test**

Create `amari-rewrite/tests/rewritable_expr.rs`:

```rust
use amari_rewrite::{Path, Rewritable};

#[derive(Clone, Debug, PartialEq, Eq)]
enum Expr {
    Lit(i64),
    Add(Box<Expr>, Box<Expr>),
}

impl Rewritable for Expr {
    fn child_count(&self) -> usize {
        match self {
            Expr::Lit(_) => 0,
            Expr::Add(_, _) => 2,
        }
    }

    fn child(&self, index: usize) -> Option<&Self> {
        match self {
            Expr::Lit(_) => None,
            Expr::Add(left, right) => match index {
                0 => Some(left),
                1 => Some(right),
                _ => None,
            },
        }
    }

    fn replace_child(&self, index: usize, replacement: Self) -> amari_rewrite::RewriteResult<Self> {
        match (self, index) {
            (Expr::Add(_, right), 0) => Ok(Expr::Add(Box::new(replacement), right.clone())),
            (Expr::Add(left, _), 1) => Ok(Expr::Add(left.clone(), Box::new(replacement))),
            _ => Err(amari_rewrite::RewriteError::InvalidChildIndex { index }),
        }
    }
}

#[test]
fn positions_include_root_and_descendants() {
    let expr = Expr::Add(Box::new(Expr::Lit(1)), Box::new(Expr::Lit(2)));
    assert_eq!(expr.positions(), vec![Path::root(), Path::from([0]), Path::from([1])]);
}

#[test]
fn subterm_reads_by_path() {
    let expr = Expr::Add(Box::new(Expr::Lit(1)), Box::new(Expr::Lit(2)));
    assert_eq!(expr.subterm(&Path::from([1])), Some(&Expr::Lit(2)));
}

#[test]
fn replace_at_replaces_nested_subterm() {
    let expr = Expr::Add(
        Box::new(Expr::Lit(1)),
        Box::new(Expr::Add(Box::new(Expr::Lit(2)), Box::new(Expr::Lit(3)))),
    );

    let rewritten = expr.replace_at(&Path::from([1, 0]), Expr::Lit(20)).unwrap();

    assert_eq!(
        rewritten,
        Expr::Add(
            Box::new(Expr::Lit(1)),
            Box::new(Expr::Add(Box::new(Expr::Lit(20)), Box::new(Expr::Lit(3)))),
        )
    );
}
```

**Step 2: Verify RED**

Run:

```bash
cargo +stable test -p amari-rewrite --test rewritable_expr
```

Expected: FAIL because `Path`, `Rewritable`, `RewriteError`, and `RewriteResult` do not exist.

**Step 3: Implement minimal core**

Create `error.rs`:

```rust
use thiserror::Error;

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum RewriteError {
    #[error("invalid child index {index}")]
    InvalidChildIndex { index: usize },
    #[error("invalid path")]
    InvalidPath,
    #[error("rewrite step limit reached")]
    StepLimitReached,
    #[error("node limit reached")]
    NodeLimitReached,
    #[error("invalid rule: {message}")]
    InvalidRule { message: alloc::string::String },
}

pub type RewriteResult<T> = Result<T, RewriteError>;
```

Create `rewritable.rs`:

```rust
extern crate alloc;

use alloc::vec::Vec;
use core::fmt::Debug;

use crate::{RewriteError, RewriteResult};

#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Path(Vec<usize>);

impl Path {
    pub fn root() -> Self { Self(Vec::new()) }
    pub fn as_slice(&self) -> &[usize] { &self.0 }
    pub fn child(&self, index: usize) -> Self {
        let mut next = self.0.clone();
        next.push(index);
        Self(next)
    }
}

impl<const N: usize> From<[usize; N]> for Path {
    fn from(value: [usize; N]) -> Self { Self(value.into()) }
}

impl From<Vec<usize>> for Path {
    fn from(value: Vec<usize>) -> Self { Self(value) }
}

pub trait Rewritable: Clone + PartialEq + Debug {
    fn child_count(&self) -> usize;
    fn child(&self, index: usize) -> Option<&Self>;
    fn replace_child(&self, index: usize, replacement: Self) -> RewriteResult<Self>;

    fn subterm(&self, path: &Path) -> Option<&Self> {
        let mut current = self;
        for &index in path.as_slice() {
            current = current.child(index)?;
        }
        Some(current)
    }

    fn replace_at(&self, path: &Path, replacement: Self) -> RewriteResult<Self> {
        match path.as_slice().split_first() {
            None => Ok(replacement),
            Some((&index, rest)) => {
                let child = self.child(index).ok_or(RewriteError::InvalidChildIndex { index })?;
                let replaced_child = child.replace_at(&Path::from(rest.to_vec()), replacement)?;
                self.replace_child(index, replaced_child)
            }
        }
    }

    fn positions(&self) -> Vec<Path> {
        fn walk<T: Rewritable>(term: &T, path: Path, out: &mut Vec<Path>) {
            out.push(path.clone());
            for index in 0..term.child_count() {
                if let Some(child) = term.child(index) {
                    walk(child, path.child(index), out);
                }
            }
        }
        let mut out = Vec::new();
        walk(self, Path::root(), &mut out);
        out
    }
}
```

Update `lib.rs` exports:

```rust
extern crate alloc;

pub mod error;
pub mod prelude;
pub mod rewritable;

pub use error::{RewriteError, RewriteResult};
pub use rewritable::{Path, Rewritable};
```

Update `prelude.rs`:

```rust
pub use crate::{Path, Rewritable, RewriteError, RewriteResult};
```

**Step 4: Verify GREEN**

Run:

```bash
cargo +stable test -p amari-rewrite --test rewritable_expr
```

Expected: PASS.

**Step 5: Commit**

```bash
git add amari-rewrite/src amari-rewrite/tests/rewritable_expr.rs
git commit -m "feat: add rewritable path core"
```

---

### Task 3: Add ARS rules, systems, and strategies

**Files:**
- Create: `amari-rewrite/src/ars/mod.rs`
- Create: `amari-rewrite/tests/ars_system.rs`
- Modify: `amari-rewrite/src/lib.rs`
- Modify: `amari-rewrite/src/prelude.rs`

**Step 1: Write failing tests**

Create `amari-rewrite/tests/ars_system.rs` using the same `Expr` fixture style as Task 2. Add tests:

```rust
#[test]
fn normalize_applies_rule_until_fixed_point() {
    let system = System::new(vec![Rule::new("add-zero-left", |expr: &Expr| match expr {
        Expr::Add(left, right) if **left == Expr::Lit(0) => Some((**right).clone()),
        _ => None,
    })]);

    let expr = Expr::Add(
        Box::new(Expr::Lit(0)),
        Box::new(Expr::Add(Box::new(Expr::Lit(0)), Box::new(Expr::Lit(5)))),
    );

    assert_eq!(system.normalize_with_limit(&expr, 8).unwrap(), Expr::Lit(5));
}

#[test]
fn all_successors_returns_every_one_step_rewrite() {
    let system = System::new(vec![Rule::new("add-zero-left", |expr: &Expr| match expr {
        Expr::Add(left, right) if **left == Expr::Lit(0) => Some((**right).clone()),
        _ => None,
    })]);

    let expr = Expr::Add(
        Box::new(Expr::Add(Box::new(Expr::Lit(0)), Box::new(Expr::Lit(1)))),
        Box::new(Expr::Add(Box::new(Expr::Lit(0)), Box::new(Expr::Lit(2)))),
    );

    let successors = system.successors(&expr).unwrap();
    assert_eq!(successors.len(), 2);
}
```

**Step 2: Verify RED**

Run:

```bash
cargo +stable test -p amari-rewrite --test ars_system
```

Expected: FAIL because `ars::{Rule, System, Strategy}` do not exist.

**Step 3: Implement minimal ARS**

Implement:

```rust
pub struct Rule<T> {
    name: alloc::string::String,
    apply: alloc::boxed::Box<dyn Fn(&T) -> Option<T>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Strategy { OuterFirst, InnerFirst, FirstRule, All }

pub struct System<T> { rules: Vec<Rule<T>> }
```

Implement `successors`, `apply_once`, and `normalize_with_limit` by traversing `positions()` and using `replace_at`.

**Step 4: Verify GREEN**

Run:

```bash
cargo +stable test -p amari-rewrite --test ars_system
```

Expected: PASS.

**Step 5: Commit**

```bash
git add amari-rewrite/src/ars amari-rewrite/src/lib.rs amari-rewrite/src/prelude.rs amari-rewrite/tests/ars_system.rs
git commit -m "feat: add abstract rewrite systems"
```

---

### Task 4: Add TRS terms and substitutions

**Files:**
- Create: `amari-rewrite/src/trs/mod.rs`
- Create: `amari-rewrite/src/trs/term.rs`
- Create: `amari-rewrite/src/trs/substitution.rs`
- Create: `amari-rewrite/tests/trs_substitution.rs`
- Modify: `amari-rewrite/src/lib.rs`
- Modify: `amari-rewrite/src/prelude.rs`

**Step 1: Write failing tests**

Create `amari-rewrite/tests/trs_substitution.rs`:

```rust
use amari_rewrite::trs::{Substitution, Term};

#[test]
fn term_rewritable_positions_include_all_nodes() {
    let term = Term::sym("add", [Term::constant("0"), Term::var("X")]);
    assert_eq!(term.positions().len(), 3);
}

#[test]
fn substitution_replaces_variables_recursively() {
    let term = Term::sym("add", [Term::constant("0"), Term::var("X")]);
    let subst = Substitution::new().with("X", Term::sym("s", [Term::constant("0")]));
    assert_eq!(
        subst.apply(&term),
        Term::sym("add", [Term::constant("0"), Term::sym("s", [Term::constant("0")])])
    );
}
```

**Step 2: Verify RED**

Run:

```bash
cargo +stable test -p amari-rewrite --test trs_substitution
```

Expected: FAIL because TRS module does not exist.

**Step 3: Implement minimal terms and substitutions**

Implement:

```rust
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Variable(Arc<str>);

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Symbol(Arc<str>);

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Term {
    Var(Variable),
    Sym(Symbol, Vec<Term>),
}
```

Use `alloc::sync::Arc` or `alloc::string::String` depending on `no_std` constraints. Implement `Rewritable` for `Term`.

Implement `Substitution` as a map from `Variable` to `Term`. In `no_std`, use `alloc::collections::BTreeMap` rather than `HashMap`.

**Step 4: Verify GREEN**

Run:

```bash
cargo +stable test -p amari-rewrite --test trs_substitution
```

Expected: PASS.

**Step 5: Commit**

```bash
git add amari-rewrite/src/trs amari-rewrite/src/lib.rs amari-rewrite/src/prelude.rs amari-rewrite/tests/trs_substitution.rs
git commit -m "feat: add TRS terms and substitutions"
```

---

### Task 5: Add TRS pattern matching and rules

**Files:**
- Create: `amari-rewrite/src/trs/matching.rs`
- Create: `amari-rewrite/src/trs/rule.rs`
- Create: `amari-rewrite/tests/trs_matching.rs`
- Modify: `amari-rewrite/src/trs/mod.rs`

**Step 1: Write failing tests**

Create `amari-rewrite/tests/trs_matching.rs`:

```rust
use amari_rewrite::trs::{match_pattern, Rule, Term};

#[test]
fn match_pattern_binds_variable() {
    let pat = Term::sym("add", [Term::constant("0"), Term::var("X")]);
    let term = Term::sym("add", [Term::constant("0"), Term::sym("s", [Term::constant("0")])]);
    let subst = match_pattern(&pat, &term).unwrap();
    assert_eq!(subst.get("X"), Some(&Term::sym("s", [Term::constant("0")])));
}

#[test]
fn nonlinear_pattern_requires_consistent_binding() {
    let pat = Term::sym("f", [Term::var("X"), Term::var("X")]);
    assert!(match_pattern(&pat, &Term::sym("f", [Term::constant("a"), Term::constant("a")])).is_some());
    assert!(match_pattern(&pat, &Term::sym("f", [Term::constant("a"), Term::constant("b")])).is_none());
}

#[test]
fn rule_rejects_rhs_variable_missing_from_lhs() {
    let err = Rule::new(Term::var("X"), Term::var("Y")).unwrap_err();
    assert!(err.to_string().contains("rhs variable"));
}

#[test]
fn rule_applies_at_root() {
    let rule = Rule::new(
        Term::sym("add", [Term::constant("0"), Term::var("X")]),
        Term::var("X"),
    ).unwrap();
    let term = Term::sym("add", [Term::constant("0"), Term::constant("a")]);
    assert_eq!(rule.apply_root(&term).unwrap(), Term::constant("a"));
}
```

**Step 2: Verify RED**

Run:

```bash
cargo +stable test -p amari-rewrite --test trs_matching
```

Expected: FAIL because matching/rules do not exist.

**Step 3: Implement matching and rules**

Implement `match_pattern` with consistent substitution merge.

Implement `Rule::new(lhs, rhs) -> RewriteResult<Rule>` by checking `rhs.variables().is_subset(lhs.variables())`.

Implement `Rule::apply_root`.

**Step 4: Verify GREEN**

Run:

```bash
cargo +stable test -p amari-rewrite --test trs_matching
```

Expected: PASS.

**Step 5: Commit**

```bash
git add amari-rewrite/src/trs amari-rewrite/tests/trs_matching.rs
git commit -m "feat: add TRS matching and rules"
```

---

### Task 6: Add TermSystem integration with ARS

**Files:**
- Modify: `amari-rewrite/src/trs/mod.rs`
- Create: `amari-rewrite/src/trs/system.rs`
- Create: `amari-rewrite/tests/trs_system.rs`

**Step 1: Write failing test**

Create `amari-rewrite/tests/trs_system.rs`:

```rust
use amari_rewrite::trs::{Rule, Term, TermSystem};

#[test]
fn peano_add_zero_normalizes_nested_term() {
    let system = TermSystem::new(vec![
        Rule::new(Term::sym("add", [Term::constant("0"), Term::var("X")]), Term::var("X")).unwrap(),
    ]);

    let term = Term::sym("add", [
        Term::constant("0"),
        Term::sym("add", [Term::constant("0"), Term::constant("a")]),
    ]);

    assert_eq!(system.normalize_with_limit(&term, 8).unwrap(), Term::constant("a"));
}
```

**Step 2: Verify RED**

Run:

```bash
cargo +stable test -p amari-rewrite --test trs_system
```

Expected: FAIL because `TermSystem` does not exist.

**Step 3: Implement TermSystem**

Implement `TermSystem { rules: Vec<trs::Rule> }` with `successors`, `apply_once`, and `normalize_with_limit` using TRS rules at every `Term` position.

**Step 4: Verify GREEN**

Run:

```bash
cargo +stable test -p amari-rewrite --test trs_system
```

Expected: PASS.

**Step 5: Commit**

```bash
git add amari-rewrite/src/trs amari-rewrite/tests/trs_system.rs
git commit -m "feat: add TRS system normalization"
```

---

### Task 7: Add inverse rewriting / bounded backward search

**Files:**
- Create: `amari-rewrite/src/inverse/mod.rs`
- Create: `amari-rewrite/tests/inverse_search.rs`
- Modify: `amari-rewrite/src/lib.rs`
- Modify: `amari-rewrite/src/prelude.rs`

**Step 1: Write failing tests**

Create `amari-rewrite/tests/inverse_search.rs`:

```rust
use amari_rewrite::{inverse::BackwardSearch, trs::{Rule, Term, TermSystem}};

#[test]
fn backward_search_finds_one_step_predecessor() {
    let system = TermSystem::new(vec![
        Rule::new(Term::sym("add", [Term::constant("0"), Term::var("X")]), Term::var("X")).unwrap(),
    ]);

    let target = Term::constant("a");
    let predecessors: Vec<_> = BackwardSearch::new(&system, target)
        .max_depth(1)
        .max_nodes(16)
        .collect();

    assert!(predecessors.contains(&Term::sym("add", [Term::constant("0"), Term::constant("a")])));
}

#[test]
fn backward_search_honors_depth_limit() {
    let system = TermSystem::new(vec![
        Rule::new(Term::sym("add", [Term::constant("0"), Term::var("X")]), Term::var("X")).unwrap(),
    ]);

    let predecessors: Vec<_> = BackwardSearch::new(&system, Term::constant("a"))
        .max_depth(0)
        .collect();

    assert!(predecessors.is_empty());
}
```

**Step 2: Verify RED**

Run:

```bash
cargo +stable test -p amari-rewrite --test inverse_search
```

Expected: FAIL because inverse module does not exist.

**Step 3: Implement bounded backward search**

Implement BFS over terms. For each current term, generate one-step predecessors by applying each TRS rule in reverse at each position: match `rule.rhs` against a subterm, instantiate `rule.lhs`, and replace at that path. Deduplicate using `BTreeSet<Term>`.

**Step 4: Verify GREEN**

Run:

```bash
cargo +stable test -p amari-rewrite --test inverse_search
```

Expected: PASS.

**Step 5: Commit**

```bash
git add amari-rewrite/src/inverse amari-rewrite/src/lib.rs amari-rewrite/src/prelude.rs amari-rewrite/tests/inverse_search.rs
git commit -m "feat: add bounded inverse rewriting"
```

---

### Task 8: Add anti-unification

**Files:**
- Create: `amari-rewrite/src/synthesis/mod.rs`
- Create: `amari-rewrite/src/synthesis/anti_unify.rs`
- Create: `amari-rewrite/tests/anti_unification.rs`
- Modify: `amari-rewrite/src/lib.rs`
- Modify: `amari-rewrite/src/prelude.rs`

**Step 1: Write failing tests**

Create `amari-rewrite/tests/anti_unification.rs`:

```rust
use amari_rewrite::{synthesis::anti_unify, trs::{match_pattern, Term}};

#[test]
fn identical_terms_generalize_to_themselves() {
    let zero = Term::constant("0");
    assert_eq!(anti_unify(&zero, &zero), zero);
}

#[test]
fn nested_terms_generalize_at_disagreement() {
    let a = Term::sym("add", [Term::constant("0"), Term::sym("s", [Term::constant("0")])]);
    let b = Term::sym("add", [Term::constant("0"), Term::sym("s", [Term::sym("s", [Term::constant("0")])])]);

    let generalized = anti_unify(&a, &b);

    match &generalized {
        Term::Sym(symbol, args) => {
            assert_eq!(symbol.as_str(), "add");
            assert_eq!(args[0], Term::constant("0"));
            assert!(matches!(&args[1], Term::Sym(s, inner) if s.as_str() == "s" && matches!(inner[0], Term::Var(_))));
        }
        _ => panic!("expected symbolic generalization"),
    }

    assert!(match_pattern(&generalized, &a).is_some());
    assert!(match_pattern(&generalized, &b).is_some());
}
```

**Step 2: Verify RED**

Run:

```bash
cargo +stable test -p amari-rewrite --test anti_unification
```

Expected: FAIL because synthesis module does not exist.

**Step 3: Implement anti-unification**

Implement a local `VarGen` with generated variable names like `_G0`, `_G1`. For two `Term::Sym` values with same symbol/arity, recurse. Otherwise return a fresh variable.

**Step 4: Verify GREEN**

Run:

```bash
cargo +stable test -p amari-rewrite --test anti_unification
```

Expected: PASS.

**Step 5: Commit**

```bash
git add amari-rewrite/src/synthesis amari-rewrite/src/lib.rs amari-rewrite/src/prelude.rs amari-rewrite/tests/anti_unification.rs
git commit -m "feat: add TRS anti-unification"
```

---

### Task 9: Add rule inference from examples

**Files:**
- Create: `amari-rewrite/src/synthesis/inference.rs`
- Create: `amari-rewrite/tests/rule_inference.rs`
- Modify: `amari-rewrite/src/synthesis/mod.rs`

**Step 1: Write failing tests**

Create `amari-rewrite/tests/rule_inference.rs`:

```rust
use amari_rewrite::{synthesis::infer_rule, trs::Term};

#[test]
fn infer_add_zero_rule_from_positive_examples() {
    let examples = vec![
        (
            Term::sym("add", [Term::constant("0"), Term::constant("a")]),
            Term::constant("a"),
        ),
        (
            Term::sym("add", [Term::constant("0"), Term::sym("s", [Term::constant("a")])]),
            Term::sym("s", [Term::constant("a")]),
        ),
    ];

    let rule = infer_rule(&examples).unwrap();

    assert_eq!(rule.apply_root(&examples[0].0).unwrap(), examples[0].1);
    assert_eq!(rule.apply_root(&examples[1].0).unwrap(), examples[1].1);
}

#[test]
fn infer_rule_rejects_empty_examples() {
    assert!(infer_rule(&[]).is_err());
}
```

**Step 2: Verify RED**

Run:

```bash
cargo +stable test -p amari-rewrite --test rule_inference
```

Expected: FAIL because `infer_rule` does not exist.

**Step 3: Implement inference**

Implement `infer_rule(&[(Term, Term)]) -> RewriteResult<trs::Rule>` by anti-unifying all LHSs and all RHSs. If RHS variables do not appear in LHS due to independent anti-unification naming, add the minimal variable alignment needed for examples to pass. If alignment becomes complex, implement a small pair anti-unification result that tracks disagreement pairs consistently across LHS/RHS examples.

**Step 4: Verify GREEN**

Run:

```bash
cargo +stable test -p amari-rewrite --test rule_inference
```

Expected: PASS.

**Step 5: Commit**

```bash
git add amari-rewrite/src/synthesis amari-rewrite/tests/rule_inference.rs
git commit -m "feat: infer rewrite rules from examples"
```

---

### Task 10: Add experimental neural and SMT scaffolding

**Files:**
- Create: `amari-rewrite/src/neural/mod.rs`
- Create: `amari-rewrite/src/smt/mod.rs`
- Create: `amari-rewrite/tests/experimental_features.rs`
- Modify: `amari-rewrite/src/lib.rs`

**Step 1: Write compile tests**

Create `amari-rewrite/tests/experimental_features.rs`:

```rust
#![cfg(any(feature = "neural", feature = "smt"))]

#[cfg(feature = "neural")]
#[test]
fn differentiable_rule_trait_can_be_implemented() {
    use amari_rewrite::neural::DifferentiableRule;

    struct IdentityRule;
    impl DifferentiableRule<f64> for IdentityRule {
        type Parameters = ();
        type Gradient = ();
        type Error = core::convert::Infallible;

        fn forward(&self, state: &f64) -> Result<f64, Self::Error> { Ok(*state) }
        fn loss(&self, predicted: &f64, target: &f64) -> Result<f64, Self::Error> {
            Ok((predicted - target).abs())
        }
    }

    assert_eq!(IdentityRule.forward(&3.0).unwrap(), 3.0);
}

#[cfg(feature = "smt")]
#[test]
fn rewrite_solver_trait_can_be_implemented() {
    use amari_rewrite::smt::RewriteSolver;

    struct TrivialSolver;
    impl RewriteSolver for TrivialSolver {
        type Term = i32;
        type Certificate = bool;
        type Error = core::convert::Infallible;

        fn prove_equivalent(&self, lhs: &i32, rhs: &i32) -> Result<bool, Self::Error> {
            Ok(lhs == rhs)
        }
    }

    assert!(TrivialSolver.prove_equivalent(&1, &1).unwrap());
}
```

**Step 2: Verify RED**

Run:

```bash
cargo +stable test -p amari-rewrite --features neural,smt --test experimental_features
```

Expected: FAIL because modules/traits do not exist.

**Step 3: Implement traits**

Create `neural::DifferentiableRule<State>` and `smt::RewriteSolver` exactly as in the design doc.

**Step 4: Verify GREEN**

Run:

```bash
cargo +stable test -p amari-rewrite --features neural,smt --test experimental_features
```

Expected: PASS.

**Step 5: Commit**

```bash
git add amari-rewrite/src/neural amari-rewrite/src/smt amari-rewrite/src/lib.rs amari-rewrite/tests/experimental_features.rs
git commit -m "feat: add experimental rewrite extension traits"
```

---

### Task 11: Add optional amari-network bridge

**Files:**
- Create: `amari-rewrite/src/network/mod.rs`
- Create: `amari-rewrite/tests/network_feature.rs`
- Modify: `amari-rewrite/src/lib.rs`

**Step 1: Write compile test**

Create `amari-rewrite/tests/network_feature.rs`:

```rust
#![cfg(feature = "network")]

use amari_rewrite::{network::RewriteGraphSummary, trs::Term};

#[test]
fn rewrite_graph_summary_tracks_terms_and_steps() {
    let summary = RewriteGraphSummary::from_trace(&[
        Term::constant("a"),
        Term::sym("f", [Term::constant("a")]),
    ]);

    assert_eq!(summary.nodes, 2);
    assert_eq!(summary.edges, 1);
}
```

**Step 2: Verify RED**

Run:

```bash
cargo +stable test -p amari-rewrite --features network --test network_feature
```

Expected: FAIL because network bridge does not exist.

**Step 3: Implement minimal network bridge**

Implement a conservative bridge that compiles with `amari-network` but does not overpromise learned behavior:

```rust
pub struct RewriteGraphSummary {
    pub nodes: usize,
    pub edges: usize,
}

impl RewriteGraphSummary {
    pub fn from_trace<T>(trace: &[T]) -> Self {
        Self { nodes: trace.len(), edges: trace.len().saturating_sub(1) }
    }
}
```

Add docs explaining that richer `GeometricNetwork` adapters are experimental future work.

**Step 4: Verify GREEN**

Run:

```bash
cargo +stable test -p amari-rewrite --features network --test network_feature
```

Expected: PASS.

**Step 5: Commit**

```bash
git add amari-rewrite/src/network amari-rewrite/src/lib.rs amari-rewrite/tests/network_feature.rs
git commit -m "feat: add optional network rewrite bridge"
```

---

### Task 12: Add examples and docs

**Files:**
- Create: `amari-rewrite/examples/symbolic_simplification.rs`
- Create: `amari-rewrite/examples/peano_trs.rs`
- Create: `amari-rewrite/examples/inverse_search.rs`
- Create: `amari-rewrite/examples/infer_rule_from_examples.rs`
- Modify: `amari-rewrite/README.md`
- Modify: `README.md`
- Modify: `Cargo.toml`

**Step 1: Add examples**

Each example should compile and be runnable with `cargo +stable run -p amari-rewrite --example <name>`.

Example topics:

- symbolic simplification with custom `Expr: Rewritable`
- Peano TRS normalization
- predecessor search from target term
- inferring `add(0, X) -> X`-style rules from examples

**Step 2: Add root workspace visibility**

Modify root `Cargo.toml`:

- Add `amari-rewrite = { workspace = true, optional = true }` to root `[dependencies]`.
- Add feature:

```toml
rewrite = ["dep:amari-rewrite"]
```

- Add `rewrite` to `full` if that is the workspace convention for new crates.

Modify root `README.md` to mention `amari-rewrite` in domain/integration crate lists and 0.23.0 roadmap notes if present.

**Step 3: Verify examples**

Run:

```bash
cargo +stable run -p amari-rewrite --example symbolic_simplification
cargo +stable run -p amari-rewrite --example peano_trs
cargo +stable run -p amari-rewrite --example inverse_search
cargo +stable run -p amari-rewrite --example infer_rule_from_examples
```

Expected: all PASS/run successfully.

**Step 4: Commit**

```bash
git add Cargo.toml README.md amari-rewrite/README.md amari-rewrite/examples
git commit -m "docs: add amari-rewrite examples"
```

---

### Task 13: Full verification and cleanup

**Files:**
- Potentially modify any files with formatting/clippy/doc issues.

**Step 1: Run formatting**

```bash
cargo +stable fmt --check
```

Expected: PASS. If it fails, run `cargo +stable fmt`, inspect diff, then rerun check.

**Step 2: Run crate tests**

```bash
cargo +stable test -p amari-rewrite --quiet
cargo +stable test -p amari-rewrite --all-features --quiet
```

Expected: PASS.

**Step 3: Run workspace check**

```bash
cargo +stable test --workspace --quiet
```

Expected: PASS.

**Step 4: Run clippy for the new crate**

```bash
cargo +stable clippy -p amari-rewrite --all-features --tests -- -D warnings
```

Expected: PASS.

**Step 5: Commit cleanup if needed**

```bash
git add <changed-files>
git commit -m "chore: verify amari-rewrite implementation"
```

---

## Final PR checklist

- [ ] `amari-rewrite` is in workspace members.
- [ ] Root crate exposes optional `rewrite` feature.
- [ ] Default build has no heavy rewrite/neural/SMT dependencies.
- [ ] `network` feature depends on `amari-network` optionally and compiles.
- [ ] ARS/TRS/inverse/synthesis tests pass.
- [ ] Experimental feature tests pass.
- [ ] Examples compile and run.
- [ ] README and crate docs honestly mark neural/SMT/network as experimental.
- [ ] Verification commands from Task 13 pass.
