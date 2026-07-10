# amari-rewrite 0.24.0 — Open Decisions

Date: 2026-07-09
Status: decisions needed before implementation planning
Context: `amari-rewrite` shipped in 0.23.0 with stable core (ARS, TRS, inverse search, anti-unification, positive-example `infer_rule`) plus experimental scaffolding. 0.24.0 fills in the deferred features. `candle` is already chosen as the tensor dependency.

---

## 1. Macros — proc-macro crate architecture

**Current state:** `macros` feature exists in Cargo.toml but is empty — no module gated on it, no proc-macro crate.

**Decision needed:**

- `derive(Rewritable)` requires a proc macro, which means a separate crate (e.g., `amari-rewrite-macros`) following the existing `amari-flynn` / `amari-flynn-macros` pattern. Agreed?
- Should `term!` and `rule!` also live in the proc-macro crate, or be `macro_rules!` in `amari-rewrite` itself?
- Is `macros` a stable feature or experimental in 0.24.0? (Proc macros are compile-time only — no runtime cost — so a case can be made for stable.)

---

## 2. SMT — solver choice and integration point

**Current state:** `RewriteSolver` trait exists (solver-agnostic) but no implementation. No solver dependency in Cargo.toml.

**Decisions needed:**

- **Which solver?** Options: z3 (mature `z3` crate, best Rust bindings), cvc5, or something lighter. z3 is the obvious first choice but adds a native build dependency.
- **Integration point:** keep the `RewriteSolver` trait in `amari-rewrite` and put the z3 implementation behind `smt` feature there, or create a separate `amari-rewrite-smt` bridge crate?
- **What does it actually do?** Minimum: `prove_equivalent` for two terms under a rule set. Stretch: counterexample generation for negative-example inference (feeds into decision 6), or `check_satisfiability` for finding terms that satisfy a pattern.
- **Experimental or stable?** Native solver dependency suggests experimental for 0.24.0.

---

## 3. Neural — candle integration shape

**Current state:** `DifferentiableRule<State>` trait exists. No tensor dependency.

**Decisions needed:**

- **Concrete adapter vs trait-only?** Option A: provide a `CandleRewriteRule` struct wrapping a `candle::Module` with encode/decode to `Term`. Option B: keep `DifferentiableRule` as the sole interface and ship examples showing how to implement it with candle. Option B is lighter but less immediately useful.
- **Integration with `Strategy`?** Should there be a `Strategy::Neural(model)` variant that selects rewrite steps via model inference? Or does neural rewriting live entirely outside the ARS/TRS systems as a separate `NeuralRewriter`?
- **Training loop?** Does `amari-rewrite` own training infrastructure (data generation from rewrite traces, loss computation, optimizer step) or is that out of scope — users bring their own trained models?
- **Experimental or stable?** `candle` is a heavy dependency. Should stay experimental.

---

## 4. Network — geometric strategy selection

**Current state:** `RewriteGraphSummary` (node/edge count) and `network_bridge_enabled()` stub. Feature `network` depends on `amari-network` + `neural`.

**Decisions needed:**

- **What does "expansion" mean concretely?** 
  - Option A: Model a rewrite search space as a `GeometricNetwork` (terms = nodes, rewrite steps = edges with rule labels) and provide graph algorithms (BFS/DFS frontier ranking, betweenness centrality for critical terms).
  - Option B: Build a `NetworkGuidedStrategy` that uses graph properties to prioritize which terms to rewrite next.
  - Option C: Feed network embeddings into the neural module for learned strategy selection (bridges decisions 3 and 4).
- **Which graph?** Should this use `amari_network::GeometricNetwork` or a simpler purpose-built graph in the network module?
- **Scope creep risk:** This could easily explode in scope. What's the MVP that ships in 0.24.0 vs what can wait?

---

## 5. Confluence / termination analysis

**Current state:** Not implemented at all. No module, no traits.

**Decisions needed:**

- **What form?** Options:
  - A: Critical pair checking — compute all critical pairs from a rule set and check joinability. Small, well-defined, classic TRS theory.
  - B: Knuth-Bendix completion — given a set of equations, attempt to produce a confluent terminating rewrite system. Much larger scope.
  - C: Termination orderings — implement LPO (lexicographic path ordering) or RPO (recursive path ordering) as termination checkers.
- **New module or ARS/TRS extension?** A `confluence` module at `amari-rewrite/src/confluence/` seems cleanest.
- **How deep?** 0.24.0 suggestion: critical pair checking + LPO. Knuth-Bendix and full completion can wait.

---

## 6. Negative-example inference — heuristic specialization

**Current state:** `infer_rules` exists and rejects rules that cover any negative example. No specialization/refinement.

**Decisions needed:**

- **What does specialization look like?** If `add(0, X) -> X` covers a negative example `add(0, 0) -> 1`, should we:
  - A: Reject outright (current behavior) — done.
  - B: Try anti-unifying the false-positive cases and synthesizing a more constrained rule.
  - C: Split into multiple rules (e.g., `add(0, X) -> X` when X ≠ 0, plus an explicit rule for the counterexample).
- **SMT integration?** Counterexample-guided refinement could use the SMT solver (decision 2) to find discriminating cases. Worth coupling these, or keep separate?
- **API:** Currently `infer_rules(positives, negatives) -> Vec<Rule>`. Does this change, or do we add new functions like `specialize_rule`, `refine_rules`?

---

## 7. Scope boundaries for 0.24.0

**Decisions needed:**

- **Which features stabilize?** Candidates: `macros` (compile-time only, low risk). SMT and neural almost certainly stay experimental.
- **Does the prelude change?** If `derive(Rewritable)` goes stable, it likely belongs in the prelude. Neural/SMT types should not.
- **Any breaking changes to the 0.23.0 stable API?** The design says the existing core is stable. 0.24.0 should be additive only.
- **WASM?** Does any of this need WASM bindings or examples-suite exposure? Probably not for 0.24.0 — `amari-discovery` is the user-facing integration surface here.
