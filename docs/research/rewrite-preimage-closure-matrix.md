# Regular preimage closure matrix (0.25 research gate, Task 23)

Status: gate artifact under independent mathematical review (ADR
0001). Companion oracle: `amari-rewrite/tests/closure_oracle.rs`.
Remediated per the first review round (R1–R9); each finding's
resolution is marked inline.

Question: for a term rewriting system (TRS) `R` and a recognizable
tree language `L` (given as a bottom-up tree automaton), when is the
**preimage** language exactly recognizable — with an effective
construction Amari can ship under its resource ceilings?

- One-step: `R⁻¹(L) = { s | ∃t ∈ L, s →_R t }`
- Finite horizon: `(R⁻¹)^≤n(L)` for an explicit `n`
- Unbounded saturation: `(R⁻¹)*(L)`

## The one-step relation (R1 resolution)

The gate certifies the standard **application** relation: a rule
applied at a position where its left side matches counts as a step,
even when the result equals the source. This is the relation the
ADR's theorem is proved for, and the relation the oracle implements.

Amari's existing `TermSystem::successors` is a **strict** search
filter: it drops identity results. Under the strict relation the
left-linear one-step class is NOT recognizability-preserving
(counterexample due to the reviewer: `R = {f(x,y) → f(y,x)}`,
`L = T(F)`, slice `K = {f(g^m(a), g^n(a))}` gives `{m ≠ n}`, whose
complement in `K` is the Example 1.2.1 diagonal). Consequences:

- The exact one-step API and its forward replay contract must use an
  application-level successor relation on `TermSystem` (Task 25 adds
  it; the existing strict filter stays as a search optimization and
  is explicitly NOT the certified relation). Pinned by
  `swap_rule_pins_application_semantics`.
- `≤n`-horizon unions and unbounded saturation COINCIDE under the
  two relations (self-steps can be deleted from any path), so every
  other cell of the matrix is unaffected. Exact-length strict layers
  are not claimed.

## Amari semantics mapping

| Literature | Amari |
|---|---|
| ground terms `T(F)` | `trs::Term` (ground; ranked symbols checked at use) |
| TRS `R` | `trs::TermSystem`; `Rule::new` enforces `Var(rhs) ⊆ Var(lhs)` (rule.rs:20) — but `Rule::new_unchecked` bypasses it (R5): the classifier/exact API boundary MUST validate the containment for every rule, and the common finite ranked alphabet of rules and language, before emitting any class or certificate (`rule_contract_is_a_boundary_obligation_not_a_constructor_fact`) |
| one-step rewrite at a position | application relation: `match_pattern` + `Substitution::apply` + `Term::replace_at` (oracle `successors()`) — see R1 resolution above |
| recognizable language `L` | `language::TreeAutomaton` (bottom-up NFTA), canonical byte certificates |
| determinization/completion | Task 20 (`determinize`, `completed`, `minimized`) |
| resource ceilings | `relation::{RelationLimits, RelationResources}`; language limits |

## The matrix

E = exact & effective (construction specified, approved class);
A = approximation-only (sound bounds / partial authority obligations);
NO = **not guaranteed recognizable for this class** (a class-wide
statement: individual TRS/language pairs in the row may still have
recognizable preimages; the conservative rejection is what the
classifier implements). Horizon zero is always exact (`(R⁻¹)^≤0(L)
= L`) for every system; NO in the horizon column means positive
horizons are not guaranteed.

| Class of `R` | One-step | Finite horizon `≤n` | Unbounded saturation |
|---|---|---|---|
| ground (no variables) | E (subcase of left-linear) | E (iterate) | E — variable-disjoint linear; also Dauchet–Tison [DT90] |
| linear, variable-disjoint (`Var(l) ∩ Var(r) = ∅` per rule) | E (left-linear subcase) | E (iterate) | E — GTT closure (see S2) |
| left-linear, shared variables allowed | E (application semantics; see S1) | E (iterate) | NO (see C2) → A |
| non-left-linear | NO (see C1) → A | NO for positive `n` | NO → A |

Amari contract note (R5 corollary): under the enforced
`Var(r) ⊆ Var(l)` boundary, variable-disjointness forces
`Var(r) = ∅` — Amari's checked `LinearVariableDisjoint` class is
**left-linear, right-ground**, with fully ground TRSs as a subcase.
The literature's fresh-RHS-variable semantics is NOT imported.

Out of scope (documented for contrast): **forward** descendants
`R*(L)`. Even the left-linear copying rule `g(x) → f(x,x)` (x
repeated on the right — left-linear, NOT linear) fails forward:
TATA Exercise 1.17.3 with `L = {g(h^n(a))}` gives correlated
descendants `{f(h^n(a), h^n(a))}`, the Example 1.2.1 diagonal
(oracle `forward_duplication_witness_is_the_real_exercise_1_17_3`).
Forward exact classes exist (linear monadic, Salomaa [Sal88]; linear
semi-monadic, Coquidé et al. [CDGV94]) but the preimage API does not
depend on them.

## S1 — left-linear one-step preimage is recognizable (assembled)

Statement, construction, and proof are carried by ADR 0001
§Construction (the reviewer verified it for the application
relation): determinize `L`'s automaton once; evaluate `rσ` from the
unique states of the matched sibling subterms (repeated right-side
variables reuse a state — no equality test, because determinism
gives each term exactly one state); recognize root instances of the
linear left side by the root-instance adaptation noted immediately
after TATA Proposition 3.4.3 (the encompassment headline's upward
transitions omitted), with the finite assignment-annotated product
recording variable-position states; track the single marked rewrite
position along the run. Requires the boundary contract
`Var(r) ⊆ Var(l)` for every rule (R5).

## S2 — unbounded saturation for linear variable-disjoint systems

TATA Proposition 3.4.7: if `R` is linear and left/right members of
each rule share no variables, then `→*_R` is recognized by a ground
tree transducer (GTT); its proof already invokes Theorem 3.2.14
(GTTs closed under transitive closure), so no second invocation is
needed. Extraction (R6 repair): Proposition 3.2.7 includes GTT
relations in Rec; with `S = →*_R ∈ Rec`,

    (R⁻¹)*(L) = { s | ∃t, (s,t) ∈ S ∩ (T(F) × L) }

via cylindrification of `L`, intersection in Rec (Proposition
3.2.9), and existential projection of the second coordinate
(Proposition 3.2.12) — all effective. Ground systems are the vacuous
subcase; for ground systems the full first-order theory of
many-step reduction is moreover decidable (Dauchet–Tison [DT90],
extensions in [Tis89]) — independent confirmation of the ground row.
(Note: decidable first-order theory alone would not imply
recognizable preimages; the GTT construction is what carries the
cell.)

Task 26 obligation (review): effectiveness of the GTT route is NOT
a license for naive layer-union saturation — `{g(a) → a}` ancestors
of `{a}` grow through every finite horizon while `g*(a)` is
recognizable. Task 26 must implement a justified
representation-level saturation (e.g., the epsilon-edge closure
behind Theorem 3.2.14) or honestly return partial frontiers on
budget exhaustion.

## C1 — counterexample: non-left-linear one-step preimage

`R = {f(x,x) → a}`, `L = {a}`: the only contraction REACHING `a` is
at the root (nested redexes rewrite to non-`a` terms), so
`R⁻¹(L) = {f(t,t) | t ∈ T(F)}` exactly. This language is not
recognizable (TATA Example 1.2.1 pumping argument: an automaton
with `k` states must equate two states on a branch of
`f(g^k(a), g^k(a))`, hence also accepts `f(g^j(a), g^k(a))`, `j<k`).
Oracle pin: `non_left_linear_one_step_preimage_is_equality_correlated`
computes the bounded preimage over the complete ordered universe and
asserts it equals the equality-correlated set, with in-universe
discriminators.

The same slice argument covers the other two NO cells for positive
horizons and saturation: intersect ancestors with
`K = {f(g^m(a), g^n(a)) : m,n ≥ 0}` (these terms have no proper
redex, so membership is exactly `m = n`, reached in one step).

Consequence: the Task 24 classifier must reject non-left-linear
systems for exact one-step/finite-horizon certificates. No exact API
may be stabilized for this class.

## C2 — counterexample: left-linear saturation with shared variables

Direct witness (R4 repair; replaces the earlier
TM-simulation/undecidability chain, which conflated non-effectivity
with non-recognizability and relied on a left-linear sharpening not
present in the supplied source):

    R = { f(g(x),g(y)) → f(x,y),  f(a,a) → a },   L = {a},
    K = {f(g^m(a), g^n(a)) : m,n ≥ 0}.

Both rules are linear; the first shares `x,y` across its sides. On
`K` only the root can rewrite: the first rule peels one `g` from
BOTH branches simultaneously; the second finishes exactly at
`f(a,a)`. Hence `(R⁻¹)*({a}) ∩ K = {f(g^n(a), g^n(a)) : n ≥ 0}` —
the non-recognizable diagonal. Since `K` is recognizable and
recognizable languages are closed under intersection, the ancestor
language itself is not recognizable. Every step is strict, so the
witness holds under both one-step semantics. Oracle pin:
`shared_variable_saturation_intersects_to_the_diagonal`.

Supporting context only (not load-bearing): a single rewrite rule
can simulate a Turing machine (TATA §2.5 remark; sharpened to one
left-linear rule by Dauchet, RTA 1989 — external to the supplied
text), and the one-step/many-step first-order theories are
undecidable for arbitrary `R` [Tre96]. Those statements concern
effectivity/decidability, not recognizability.

Consequence: outside the linear variable-disjoint class, saturation
is approximation/partial-authority only.

## Approximation obligations (cells marked A)

- **Lower bound (exact under-approximation):** for the approved
  left-linear setting, iterate one-step preimages to an explicit
  horizon `n`; the union is a certified subset of the saturation.
  For non-left-linear systems the one-step step is not exact-regular
  at all, so under-approximations need explicitly replayed finite
  witnesses (as the Task 27 plan requires) — not the automaton
  construction.
- **Upper bound (certified over-approximation):** any abstraction
  must be provably a superset, and every linearization/merge/widen
  event must be recorded in the certificate. Design is Task 27
  research; this gate approves no specific upper construction.
- **Partial authority:** saturation attempts that exceed budgets
  return the partial frontier plus typed limit information — never a
  silently truncated "fixpoint". Exact certificates are emitted only
  after construction and canonicalization complete under budgets
  (Task 26).

## Resource obligations (R7)

Beyond determinization blowup, the left-linear construction has an
independent exponential parameter `k = |Var(l)|`: an explicitly
materialized `eval_r` table has `|Q|^k` entries (construction cost
`O(|r| · |Q|^k)`), and the annotated matcher needs up to
`Σ_(ρ,p) |Q|^{vars below p}` matching states. The symbol-rank
ceiling 16 does NOT bound `k` (a binary LHS skeleton can carry many
distinct variable leaves); existing term-size limits bound `k` only
coarsely. Task 25 must preflight checked exponentiation, tuple
enumeration, matching states/transitions, and epsilon elimination
BEFORE allocation or unmetered work; lazy/memoized evaluation is a
permitted alternative but still needs bounded accounting. No
additional finite-`k` restriction is needed for recognizability —
this is a resource/certificate obligation, not a theorem
precondition.

## References

- [TATA] Comon, Dauchet, Gilleron, Jacquemard, Lugiez, Löding,
  Tison, Tommasi. *Tree Automata Techniques and Applications*,
  2008 (hal-03367725). Cited: Example 1.2.1; Theorems 1.4.3, 1.4.4;
  Propositions 3.2.7, 3.2.9, 3.2.12, 3.4.3 (and the root-instance
  adaptation immediately following its proof), 3.4.7 (whose proof
  invokes Theorem 3.2.14); Exercise 1.17; §2.5 (context only).
- [DT90] Dauchet, Tison. *The theory of ground rewrite systems is
  decidable*. LICS 1990 (bibliographic remarks in TATA §3.6.6,
  referring to §3.4.3, in the supplied edition).
- [Tis89] Tison. *Fair termination is decidable for ground systems*.
  RTA 1989 (TATA §3.6.6).
- [GB85] Gallier, Book. *Reductions in tree replacement systems*.
  TCS 1985 — `IRR(S)` recognizable for left-linear `S` (TATA §1.9).
- [Sal88] Salomaa. *Deterministic tree pushdown automata and
  monadic tree rewriting systems*. JCSS 1988 (TATA Exercise 1.17.4).
- [CDGV94] Coquidé, Dauchet, Gilleron, Vágvölgyi. *Bottom-up tree
  pushdown automata: classification and connection with rewrite
  systems*. TCS 1994 (TATA §1.9).
- [Tre96] Treinen. *The first-order theory of one-step rewriting is
  undecidable*. RTA 1996 (TATA §3.6.6; context only — it is not a
  left-linear reachability theorem).
- [Dauchet89] Dauchet. *Simulation of Turing machines by a
  left-linear rewrite rule*. RTA 1989 (external to the supplied
  text; context only).

Reviewer instruction: verify each citation's statement and
preconditions against the source, verify the oracle pins match the
matrix claims (including universe cardinality and in-universe
discriminators), verify the R1 semantics resolution and the C2
direct witness, and verify the S1 construction/proof in ADR 0001 —
not just the bibliography.
