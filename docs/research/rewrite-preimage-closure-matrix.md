# Regular preimage closure matrix (0.25 research gate, Task 23)

Status: gate artifact under independent mathematical review (ADR
0001). Companion oracle: `amari-rewrite/tests/closure_oracle.rs`.

Question: for a term rewriting system (TRS) `R` and a recognizable
tree language `L` (given as a bottom-up tree automaton), when is the
**preimage** language exactly recognizable — with an effective
construction Amari can ship under its resource ceilings?

- One-step: `R⁻¹(L) = { s | ∃t ∈ L, s →_R t }`
- Finite horizon: `(R⁻¹)^≤n(L)` for an explicit `n`
- Unbounded saturation: `(R⁻¹)*(L)`

## Amari semantics mapping

| Literature | Amari |
|---|---|
| ground terms `T(F)` | `trs::Term` (ground; ranked symbols checked at use) |
| TRS `R` | `trs::TermSystem`; `Rule::new` enforces `Var(rhs) ⊆ Var(lhs)` (rule.rs:19) — pinned by `rule_rhs_variable_contract_holds` |
| one-step rewrite at a position | `match_pattern` + `Substitution::apply` + `Term::replace_at` (oracle `successors()`) |
| recognizable language `L` | `language::TreeAutomaton` (bottom-up NFTA), canonical byte certificates |
| determinization/completion | Task 20 (`determinize`, `completed`, `minimized`) |
| resource ceilings | `relation::{RelationLimits, RelationResources}`; language limits |

## The matrix

E = exact & effective (construction specified, approved class);
A = approximation-only (sound bounds / partial authority obligations);
NO = no exact construction possible (counterexample pinned).

| Class of `R` | One-step | Finite horizon | Unbounded saturation |
|---|---|---|---|
| ground (no variables) | E (subcase of left-linear) | E (iterate) | E — variable-disjoint linear; also Dauchet–Tison [DT90] |
| linear, variable-disjoint (`Var(l) ∩ Var(r) = ∅` per rule) | E (left-linear subcase) | E (iterate) | E — GTT closure (see S2) |
| left-linear, shared variables allowed | E (see S1) | E (iterate) | NO (see C2) → A |
| non-left-linear | NO (see C1) → A | NO | NO → A |

Out of scope (documented for contrast): **forward** descendants
`R*(L)`. Even linear rules fail forward (duplication:
`g(x) → f(x,x)`; TATA Exercise 1.17.3; oracle
`duplicating_rule_grows_leaf_correlated_descendants`). Forward exact
classes exist (linear monadic, Salomaa [Sal88]; linear semi-monadic,
Coquidé et al. [CDGV94]) but the preimage API does not depend on
them.

## S1 — left-linear one-step preimage is recognizable (assembled)

Statement, construction, and proof sketch are carried by ADR 0001
§Construction (the reviewer verifies that, not this summary):
determinize `L`'s automaton once; evaluate `rσ` from the unique
states of the matched sibling subterms (repeated right-side
variables reuse a state — no equality test, because determinism
gives each term exactly one state); recognize instances of the
linear left side by the standard product (TATA Prop. 3.4.3's
automaton for instances of a linear term); track the single marked
rewrite position along the run. Requires the TRS contract
`Var(r) ⊆ Var(l)` (Amari enforces it at `Rule::new`).

## S2 — unbounded saturation for linear variable-disjoint systems

TATA Proposition 3.4.7: if `R` is linear and left/right members of
each rule share no variables, then `→*_R` is recognized by a ground
tree transducer (GTT). TATA Theorem 3.2.14: GTT-recognized relations
are closed under transitive closure. The inverse image of a
recognizable language under a GTT-recognized relation is
recognizable and effective via relational composition (Prop. 3.2.16),
Boolean closure (Prop. 3.2.9), and projection/cylindrification
(Prop. 3.2.12). Ground systems are the vacuous subcase; for ground
systems the full first-order theory of many-step reduction is
moreover decidable (Dauchet–Tison [DT90], extensions in [Tis89]) —
independent confirmation of the ground row.

## C1 — counterexample: non-left-linear one-step preimage

`R = {f(x,x) → a}`, `L = {a}`: `R⁻¹(L) = {f(t,t) | t ∈ T(F)}`
exactly (the only redex is at the root). This language is not
recognizable (TATA Example 1.2.1 pumping argument: an automaton
with `k` states must equate two states on a branch of
`f(g^k(a), g^k(a))`, hence also accepts `f(g^j(a), g^k(a))`, `j<k`).
Oracle pin: `non_left_linear_one_step_preimage_is_equality_correlated`
computes the bounded preimage exhaustively and asserts it equals the
equality-correlated set, with discriminators.

Consequence: the Task 24 classifier must reject non-left-linear
systems for exact one-step/finite-horizon certificates. No exact API
may be stabilized for this cell.

## C2 — counterexample: left-linear saturation with shared variables

Unbounded ancestors under left-linear systems with left/right shared
variables are not recognizable in general: a single rewrite rule can
simulate a Turing machine (TATA §2.5 remark; sharpened to one
left-linear rule by Dauchet, RTA 1989), and both the one-step and
many-step theories are undecidable for arbitrary `R` [Tre96]. If
`(R⁻¹)*(L)` were always effectively recognizable, reachability
(`s ∈ (R⁻¹)*({t})`) would be decidable for these systems —
contradiction. The growth witness
`duplicating_rule_grows_leaf_correlated_descendants` shows the
correlation mechanism concretely on the forward side.

Consequence: outside the linear variable-disjoint class, saturation
is approximation/partial-authority only.

## Approximation obligations (cells marked A)

- **Lower bound (exact under-approximation):** iterate one-step
  preimages to an explicit horizon `n` (each step exact for
  left-linear `R`); the union is a certified subset of the
  saturation. Task 27.
- **Upper bound (certified over-approximation):** any abstraction
  must be provably a superset, and every linearization/merge/widen
  event must be recorded in the certificate. Design is Task 27
  research; this gate approves no specific upper construction.
- **Partial authority:** saturation attempts that exceed budgets
  return the partial frontier plus typed limit information — never a
  silently truncated "fixpoint". Task 26.

## References

- [TATA] Comon, Dauchet, Gilleron, Jacquemard, Lugiez, Löding,
  Tison, Tommasi. *Tree Automata Techniques and Applications*,
  2008 (hal-03367725). Cited: Example 1.2.1; Theorems 1.4.3, 1.4.4,
  3.2.14; Propositions 3.2.9, 3.2.12, 3.2.16, 3.4.3, 3.4.7;
  Exercise 1.17; §2.5.
- [DT90] Dauchet, Tison. *The theory of ground rewrite systems is
  decidable*. LICS 1990 (as cited in TATA §3.4.3 notes).
- [Tis89] Tison. *Fair termination is decidable for ground systems*.
  RTA 1989 (as cited in TATA §3.4.3 notes).
- [GB85] Gallier, Book. *Reductions in tree replacement systems*.
  TCS 1985 — `IRR(S)` recognizable for left-linear `S` (TATA §1.9).
- [Sal88] Salomaa. *Deterministic tree pushdown automata and
  monadic tree rewriting systems*. JCSS 1988 (TATA Exercise 1.17.4).
- [CDGV94] Coquidé, Dauchet, Gilleron, Vágvölgyi. *Bottom-up tree
  pushdown automata: classification and connection with rewrite
  systems*. TCS 1994 (TATA §1.9).
- [Tre96] Treinen. *The first-order theory of one-step rewriting is
  undecidable*. RTA 1996 (TATA §3.4.3 notes).
- [Dauchet89] Dauchet. *Simulation of Turing machines by a
  left-linear rewrite rule*. RTA 1989.

Reviewer instruction: verify each citation's statement and
preconditions against the source, verify the oracle pins match the
matrix claims, and verify the S1 construction/proof in ADR 0001 —
not just the bibliography.
