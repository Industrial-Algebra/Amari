# ADR 0001: Exact regular preimage scope for tree rewriting

- Status: **Proposed** — pending independent mathematical review
  (0.25 Task 23 gate; no Task 24–26 implementation may begin until
  no Critical/Important finding remains).
- Date: 2026-09-28
- Gate artifacts: `docs/research/rewrite-preimage-closure-matrix.md`
  (matrix, citations, counterexamples), `amari-rewrite/tests/
  closure_oracle.rs` (bounded concrete oracle pins).

## Context

The 0.25 rewrite/inverse-rewrite program wants an API that computes
the **preimage** of a recognizable tree language under rewriting,
exactly and with certificates. Cohort 4 shipped the language
infrastructure (NFTA, grammars, determinize/minimize, canonical
certificates, metered resources). Unrestricted exact-regular claims
are false (matrix §C1–C2), so the exact API surface must be scoped
to classes with a theorem and preconditions behind them.

## Decision

Approve exactly two classes for exact preimage APIs:

1. **Left-linear one-step and finite-horizon preimages.** For any
   finite TRS `R` whose left-hand sides are linear (each variable at
   most once), `R⁻¹(L)` is recognizable, effectively; finite-horizon
   `(R⁻¹)^≤n(L)` follows by iteration. Construction and proof in
   §Construction below. Ground systems are a subcase.
2. **Unbounded saturation for linear variable-disjoint systems.**
   For `R` linear with `Var(l) ∩ Var(r) = ∅` per rule, `(R⁻¹)*(L)`
   is recognizable, effectively (TATA Prop. 3.4.7 + Thm. 3.2.14 with
   relational closure under composition/projection/Boolean ops:
   Props. 3.2.16, 3.2.12, 3.2.9). Ground systems are the vacuous
   subcase, independently confirmed by Dauchet–Tison [DT90].

Everything outside these classes is **classifiers + sound
approximation + partial authority only** (matrix §Obligations):
non-left-linear systems get no exact one-step API (C1); left-linear
systems with shared variables get no exact saturation API (C2).

## Construction (left-linear one-step preimage)

**Theorem (assembled).** Let `F` be a finite ranked alphabet, `R` a
finite TRS over `F` with every rule left-linear and
`Var(r) ⊆ Var(l)` per rule, and `L ⊆ T(F)` recognizable. Then
`R⁻¹(L)` is recognizable, effectively.

**Preprocessing.** Let `A = (Q, F, Δ, Qf)` be a complete
deterministic bottom-up automaton for `L` (exists by the standard
determinization; Amari: `determinize` + `completed`, Task 20). Write
`δ(u) ∈ Q` for the unique state `A` assigns to a ground term `u`.

**Key table.** For a rule `ρ = (l → r)` with `Var(l) = {x1,…,xk}`
(distinct; `l` linear) define `eval_r : Q^k → Q` by evaluating `r`
bottom-up where each leaf `xi` is replaced by the state `qi` and
each function symbol by its `Δ` transition. Because `A` is
deterministic, every ground term has exactly one state, so for every
ground substitution `σ`:

    (⋆)  δ(rσ) = eval_r(δ(σ(x1)), …, δ(σ(xk)))

by induction on the structure of `r`: at a variable leaf (even a
repeated one) the claim is the same single state; at a function
node it is the determinism of `Δ`. **No equality test between
sibling subtrees is ever performed** — repetition of variables on
the right is handled by reusing a state value, which is exactly why
left-linearity (not right-linearity) is the load-bearing
precondition, and why the non-left-linear counterexample C1 does not
contradict this theorem.

**The preimage automaton `B`.** `B` runs `A` on `s` while tracking
whether the single rewrite position has been passed:

- off the marked path, `B` runs `A`'s transitions directly;
- at the marked position `p`, `B` runs a product that (i) checks
  `s|p` is an instance of `l` — recognizable for linear `l` by the
  TATA Prop. 3.4.3 instance automaton — while (ii) reading the
  `A`-states of the subterms at `l`'s `k` (distinct) variable
  positions, and (iii) emitting `eval_r` of that state tuple as the
  state the context sees at `p`;
- above `p`, `B` resumes `A`'s transitions with exactly one marked
  child;
- acceptance: the root state is in `Qf` and the mark was used.

`B` is finite (the mark is Boolean; the products are over `Q` and
the finite skeletons of finitely many left-hand sides) and
effectively constructible within resource ceilings that the Task 25
implementation must budget (determinization can explode
exponentially; per-horizon canonicalization and `RelationResources`
budgets apply).

**Soundness.** If `B` accepts `s` with mark at `p`, then
`s = C[lσ]` for some ground `σ` (by (i)), and `B`'s run above `p`
is an `A`-run on `C[δ(rσ)]` (by (iii) and (⋆)) ending in `Qf`; hence
`t = C[rσ] ∈ L` and `s →_R t`. ∎

**Completeness.** If `s →_R t ∈ L` by rule `ρ` at position `p` with
substitution `σ`, then `s|p = lσ`; the instance automaton accepts
`s|p` recording states `δ(σ(xi))`; the emitted state is
`eval_r(…) = δ(rσ)` by (⋆); the accepting `A`-run on `t = C[rσ]`
above `p` lifts to a `B`-run on `s`; `t ∈ L` gives acceptance. ∎

**Finite horizon.** `(R⁻¹)^≤n(L) = ⋃_{i≤n} (R⁻¹)^i(L)`: iterate the
construction (`R⁻¹` of a recognizable language is recognizable), take
the finite union (Prop. 3.2.9), canonicalize per step. Each iterate
is exact; the union is exact.

## Alternatives considered

- **Claim exact saturation for all left-linear systems.** Rejected:
  C2 (TM-simulation, undecidability [Tre96]).
- **Claim exact one-step under right-linearity conditions instead.**
  Misdiagnosed: repetition on the right is harmless for preimages
  (state reuse); repetition on the left is fatal (equality
  correlation, C1). The matrix records the asymmetry.
- **GTT route for one-step.** Prop. 3.4.7 needs variable
  disjointness, which would exclude shared-variable workhorses
  (`g(x) → f(x,x)`); the assembled construction needs only
  left-linearity. The GTT route is retained exactly where its
  precondition holds: saturation.
- **Stabilize the API now, gate later.** Rejected by policy; this
  ADR is the gate.

## Consequences

- Task 24 classifier taxonomy (exhaustive, no heuristic promotion):
  `Ground`, `LinearVariableDisjoint`, `LeftLinearShared`,
  `NonLeftLinear`; certificates bind system/language/classifier/
  construction/horizon/limit/result hashes.
- Task 25 implements §Construction for approved classes only;
  differential-tests against `closure_oracle.rs` (its P1 row is the
  shared-variable seed).
- Task 26 implements saturation for the linear variable-disjoint
  class (incl. ground; oracle P2 row pins the fixpoint behavior);
  anything else returns partial frontier + typed limits.
- Task 27 owns lower/upper approximations under the matrix's
  obligations; no upper construction is approved here.
- If the independent reviewer finds a Critical/Important defect that
  invalidates a class, Tasks 24–26 are replanned around what
  survives — possibly classifiers + bounds only.

## References

As in `rewrite-preimage-closure-matrix.md` (TATA with theorem
numbers; [DT90], [Tis89], [GB85], [Sal88], [CDGV94], [Tre96],
[Dauchet89]).
