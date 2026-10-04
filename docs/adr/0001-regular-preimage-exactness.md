# ADR 0001: Exact regular preimage scope for tree rewriting

- Status: **Proposed** — second review round (remediated per the
  first independent review: findings R1–R9 resolved inline; 0.25
  Task 23 gate — no Task 24–26 implementation may begin until no
  Critical/Important finding remains).
- Date: 2026-09-28 (round 1), remediated 2026-10-04
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

## The one-step relation (R1)

This gate certifies the standard **application** relation: a rule
applied at a position where its left side matches counts as a step,
even when the result equals the source term. Amari's existing
`TermSystem::successors` filters identity results (a search
optimization) and is NOT the certified relation: under the strict
relation the left-linear one-step class is not
recognizability-preserving (swap-rule counterexample, matrix §R1).
Accordingly:

- the exact one-step API and its forward replay contract use an
  application-level successor relation on `TermSystem` (Task 25
  adds it alongside the untouched strict filter);
- `≤n`-horizon unions and unbounded saturation coincide under both
  relations (self-steps deletable from paths), so no other matrix
  cell is affected; exact-length strict layers are not claimed.

## Decision

Approve exactly two classes for exact preimage APIs:

1. **Left-linear one-step and finite-horizon preimages** (under the
   application relation). For any finite TRS `R` whose left-hand
   sides are linear, `R⁻¹(L)` is recognizable, effectively;
   finite-horizon `(R⁻¹)^≤n(L)` follows by iteration and finite
   union. Construction and proof in §Construction below (verified by
   the first review for this relation, including erased variables,
   single-variable right sides, and ground rules). Ground systems
   are a subcase.
2. **Unbounded saturation for linear variable-disjoint systems.**
   For `R` linear with `Var(l) ∩ Var(r) = ∅` per rule, `(R⁻¹)*(L)`
   is recognizable, effectively: TATA Prop. 3.4.7 yields a GTT for
   `→*_R` (its proof already invoking Thm. 3.2.14); Prop. 3.2.7
   includes GTT relations in Rec; extraction by cylindrification,
   Rec intersection (Prop. 3.2.9), and projection (Prop. 3.2.12).
   Under Amari's enforced `Var(r) ⊆ Var(l)` boundary this class is
   exactly **left-linear, right-ground** (disjointness forces
   `Var(r) = ∅`), with fully ground systems as a subcase —
   independently confirmed by Dauchet–Tison [DT90].

Everything outside these classes is **classifiers + sound
approximation + partial authority only** (matrix §Obligations):
non-left-linear systems get no exact one-step API (C1); left-linear
systems with shared variables get no exact saturation API (C2).

## Input contract (R5)

The theorems quantify over TRSs on ground terms `T(F)` over a common
finite ranked alphabet. Admitted Amari values do not guarantee this:
`Rule::new_unchecked` bypasses the `Var(r) ⊆ Var(l)` check, and
rewriting with such rules produces nonground terms (oracle
`rule_contract_is_a_boundary_obligation_not_a_constructor_fact`).
Therefore the classifier/exact API boundary MUST, before emitting
any class or certificate:

1. validate `Var(r) ⊆ Var(l)` for EVERY rule (checked or not),
   returning a typed invalid/unsupported outcome otherwise;
2. validate that rules and the language automaton share one finite
   ranked alphabet, and complete the determinized automaton over
   THAT alphabet (completion is load-bearing: erased-variable
   arguments may have no accepting run in a partial automaton but
   still need an evaluation state).

## Construction (left-linear one-step preimage)

**Theorem (assembled; verified by review round 1 for the application
relation).** Let `F` be a finite ranked alphabet, `R` a finite TRS
over `F` with every rule left-linear and `Var(r) ⊆ Var(l)` per rule,
and `L ⊆ T(F)` recognizable. Then `R⁻¹(L)` is recognizable,
effectively.

**Preprocessing.** Let `A = (Q, F, Δ, Qf)` be a complete
deterministic bottom-up automaton for `L` (Amari: `determinize` +
`completed`, Task 20). Write `δ(u)` for the unique state `A`
assigns to a ground term `u`.

**Key table.** For a rule `ρ = (l → r)` with `Var(l) = {x1,…,xk}`
(distinct; `l` linear) define `eval_r : Q^k → Q` by evaluating `r`
bottom-up where each leaf `xi` is replaced by the state `qi` and
each function symbol by its `Δ` transition. By induction on `r`'s
structure, for every ground substitution `σ`:

    (⋆)  δ(rσ) = eval_r(δ(σ(x1)), …, δ(σ(xk)))

— at a variable leaf (even repeated or erased elsewhere) the claim
is the same single state; at a function node it is the determinism
of `Δ`. **No equality test between sibling subtrees is ever
performed**: repetition on the right reuses a state value. This is
why left-linearity (not right-linearity) is the load-bearing
precondition, and why the non-left-linear counterexample C1 does not
contradict the theorem. The first review confirmed (⋆) including
erased variables, arbitrarily many right-side occurrences,
single-variable right sides, and ground rules (`k=0`).

**The preimage automaton `B`** (explicit finite epsilon-NFTA
realization confirmed by review round 1): disjoint ordinary states
`U_q` simulating `A`, and rewritten states `V_q`; for each rule and
each left-side skeleton position `p`, matching states
`M_(ρ,p,α)` where `α` assigns a state in `Q` to each distinct
variable below `p` — at a variable leaf an epsilon transition from
`U_q` records `{xi ↦ q}`; at a function skeleton node the exact
symbol is required and child assignments combined (left-linearity
makes the variable sets disjoint, so combination loses no tree
equality). A conversion epsilon transition maps `M_(ρ,root,α)` to
`V_(eval_r(α))`; matching states consume only `U` states (a marked
redex contains no earlier rewrite). Above the mark, transitions have
exactly one `V` child and apply `A`'s transition; acceptance at
`V_q`, `q ∈ Qf`. Finitely many states/transitions; epsilon
elimination is effective.

**Soundness.** A successful `V` run contains exactly one conversion
at some position `p`; the matching-state induction gives a single
`σ` with `s|p = lσ`; (⋆) makes the emitted state `δ(rσ)`; siblings
are evaluated unchanged, so the final state is `δ(C[rσ]) ∈ Qf`.
Hence `s = C[lσ] →_R C[rσ] ∈ L` (an application step; the result may
equal the source, which the relation admits). ∎

**Completeness.** Given an application step `C[lσ] → C[rσ] ∈ L`,
compute the `U` states of all matched subtrees, use the matching
transitions for that rule, convert once to `V_(δ(rσ))`, and follow
the unique accepting context evaluation of `A`. ∎

**Finite horizon.** `(R⁻¹)^≤n(L) = ⋃_{i≤n} (R⁻¹)^i(L)`: iterate the
construction (each iterate recognizable), finite union (Prop.
3.2.9), canonicalize per step. Exact for `n = 0` (just `L`) and all
finite `n`. Canonicalization preserves the language; it is not
itself a proof of stabilization.

## Resource obligations (R7)

Beyond determinization blowup, the construction has an independent
exponential parameter `k = |Var(l)|`: an explicitly materialized
`eval_r` table has `|Q|^k` entries (`O(|r| · |Q|^k)` construction),
and the annotated matcher needs up to `Σ_(ρ,p) |Q|^{vars below p}`
matching states. The symbol-rank ceiling 16 does NOT bound `k`.
Task 25 must preflight checked exponentiation, tuple enumeration,
matching states/transitions, and epsilon elimination BEFORE
allocation or unmetered work; lazy/memoized evaluation is permitted
but must still be accountably bounded. No additional finite-`k`
restriction is needed for recognizability — this is a resource
obligation, not a theorem precondition. Exact certificates are
emitted only after construction and canonicalization complete under
budgets; over-limit results are typed limit outcomes, never silent
truncations.

## Alternatives considered

- **Keep the strict `TermSystem::successors` relation for the exact
  one-step API.** Rejected (R1): the strict left-linear class is not
  recognizability-preserving (swap-rule slice complements to the
  Example 1.2.1 diagonal); re-scoping to non-unifiable rule pairs
  was considered and rejected as needlessly shrinking the class
  (permutation/involution rules are legitimate) when the standard
  application relation already admits the full left-linear theorem.
- **Claim exact saturation for all left-linear systems.** Rejected:
  C2 (direct simultaneous-peeling witness, matrix §C2).
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
  `Ground`, `LinearVariableDisjoint` (= left-linear right-ground
  under the input contract), `LeftLinearShared`,
  `NonLeftLinear`; the boundary validations of §Input contract run
  BEFORE classification; certificates bind system/language/
  classifier/construction/horizon/limit/result hashes.
- Task 25 implements §Construction for approved classes only;
  provides the application-level `TermSystem` successor relation for
  replay; keeps identity-instance regressions (oracle swap test);
  preflights the §Resource obligations; differential-tests against
  `closure_oracle.rs` (P1 row is the shared-variable seed).
- Task 26 implements saturation for the linear variable-disjoint
  class via a justified representation-level fixpoint (e.g., the
  epsilon-edge closure behind Thm. 3.2.14) — NOT naive layer-union
  (matrix §S2 warning); anything else returns partial frontier +
  typed limits; certificates only after completed construction.
  Oracle P2 row pins the ground fixpoint behavior.
- Task 27 owns lower/upper approximations under the matrix's
  obligations (lower bounds for non-left-linear systems need
  replayed finite witnesses, not the automaton step); no upper
  construction is approved here.
- If the independent reviewer finds a Critical/Important defect that
  invalidates a class, Tasks 24–26 are replanned around what
  survives — possibly classifiers + bounds only.

## References

As in `rewrite-preimage-closure-matrix.md` (TATA with theorem
numbers; bibliographic remarks for [DT90], [Tis89], [Tre96] located
at TATA §3.6.6 in the supplied edition; [GB85], [Sal88], [CDGV94];
[Dauchet89] external, context only).
