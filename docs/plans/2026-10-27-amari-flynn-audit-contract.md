# Amari Flynn Audit & Remediation Contract — 0.25.0 inclusion

**Status:** DECIDED (operator, 2026-10-27/28) — D1(c), D2(c) as R1-amended
(certified-enclosure default), D3(b), D4(a), D5(a), D6(a). Implementation
may proceed on this document.
**Date:** 2026-10-27
**Scope:** `amari-flynn` 1,509 src LOC + `amari-flynn-macros` 526 LOC (leaf crates; deps: macros only)
**Operator decision (2026-10-27):** pulled forward from programme Phase B into 0.25.0.
**Doctrine:** correctness first; compatibility is not a constraint; every finding repaired
or explicitly dispositioned.

## 1. Census

| Item | Value |
|---|---|
| Files | 11 src (`prob`, `contracts`, `distributions`, `backend/{monte_carlo,smt,why3}`, `statistical/{bounds,estimators}`, `lib`) + macros lib |
| Integration tests | 2 files, 353 lines (`macro_tests`, `smt_tests`) |
| Inline unit tests | present in every module |
| Features | `default = ["std"]`, `std = []`; `geometric` TODO-commented out |
| Sole Rust consumer | karpal-verify (feature `amari`, optional dep `^0.24.0`); re-exports `prob_requires`/`prob_ensures`/`ensures_expected`, wraps Monte Carlo + all four SMT obligation builders |
| Runtime deps | `amari-flynn-macros`, `rand`, `rand_distr` (no z3 — SMT backend is text generation only) |

## 2. Findings

Severity scale: P1 = the artifact claims a guarantee it does not provide.

### F1 (P1) — Systemic verification-direction inversion
Three mutually inconsistent semantics for the SAME bound parameter, and the
implementation is wrong under both intended ones:

- Generated doc string: "Requires `cond` with P >= 0.95" (condition HOLDS).
- Attribute docs: "probability_bound: Maximum probability the condition is
  **violated**".
- Implementation (`macros → MonteCarloVerifier::verify_probability_bound`):
  checks **P(cond) ≤ bound** — neither reading.

Consequences: a precondition that holds 99% of the time (documented P ≥ 0.95)
reports **Violated**; one that holds 2% of the time reports **Verified**.
Every generated `verify_*_precondition`/`verify_*_postcondition` helper asserts
non-compliance. `SmtProofObligation::verify_with_monte_carlo` (smt.rs) repeats
the inversion for Pre/PostconditionBound with a vacuous `|| true` predicate
and NO way to inject a real one. **The inversion is codified**: `macro_tests.rs`
test_prob_requires_two_params asserts Violated for an always-true precondition
with the comment "the verifier correctly reports Violated".

Evidence: `amari-flynn-macros/src/lib.rs` (prob_requires/prob_ensures helper
generation), `amari-flynn/src/backend/monte_carlo.rs` (verify_probability_bound
API), `amari-flynn/src/backend/smt.rs` (verify_with_monte_carlo),
`amari-flynn/tests/macro_tests.rs:58-77`.

### F2 (P1) — SMT-LIB2 output is not valid QF_NRA
`(set-logic QF_NRA)` scripts embed `(exp ...)`. SMT-LIB2's QF_NRA
(quantifier-free nonlinear real arithmetic) contains no `exp`. Z3/CVC5 reject
the script as written (unknown constant/function). The module doc claims
"QF_NRA ... supports the exponential" — false. No test ever invokes a solver;
all assertions are string-contains checks. The backend's central promise —
"output can be used ... for external solvers" — has never been validated.
A z3 4.15.3 binary is available locally (creusot install) for solver-validated
tests.

Evidence: `amari-flynn/src/backend/smt.rs` (to_smtlib2, hoeffding_obligation,
expected_value_obligation; module doc; all tests).

### F3 (P2) — Monte Carlo epsilon does not implement the stated 95% bound
Both `verify_probability_bound` and `estimate_probability` use
ε = √(2·ln(1/δ)/n). Two-sided 95% Hoeffding is ε = √(ln(2/δ)/(2n)).
At n=1000, δ=0.05: implemented 0.0774 vs true 0.0429 — **1.802× wider**.
Direction is conservative (intervals wider than claimed, "Verified" harder),
so no unsoundness — but the stated confidence is false (actual ≥ ~99.8%) and
the crate's own `statistical::bounds::hoeffding_bound` gives the correct form
to invert. Also `MonteCarloVerifier::new(0)` → 0/0 = NaN → silently
`Inconclusive` (undocumented degenerate).

### F4 (P2) — confidence_z_score: window misclassification AND silent 1.960
substitution
Three z-values (90/95/99%) with ±0.01 windows; everything else — 0.80,
even 0.5 — silently gets the 95% z. Two distinct defects (R1-corrected):
(1) the ±0.01 windows MISCLASSIFY nearby levels — 0.999 requests the 99.9%
interval and receive the 99% one (|0.999−0.99| < 0.01); (2) the fallback
substitutes 1.960 for everything else. A caller receives a weaker interval
than requested, labeled with the requested level: a false-guarantee defect
in the UNSOUND direction.

### F5 (P2) — chernoff_bound: wrong formula domain, silently
Implements P ≤ exp(−μδ²/3), the additive-Chernoff form valid for δ ∈ (0,1],
for ALL δ with no check and no documentation of μ (expectation of the sum,
not a probability). For δ > 1 the bound is invalid. `_n` unused (correct for
the multiplicative form but undocumented).

### F6 (P2) — Constructor invariant gaps (fail-late)
- `Uniform::new(min, max)` does not validate `min < max`; the panic surfaces
  at SAMPLE time (`random_range` on an empty range) far from the cause.
- `MonteCarloVerifier::new(0)` accepted (see F3 NaN).

### F7 (P2) — The no_std claim is false: `--no-default-features` does not compile
`#![cfg_attr(not(feature = "std"), no_std)]` but the no-std configuration has
unresolved `std` imports and bare `format!` (needs alloc). The configuration
has never been built — same defect class as amari-core's no-std host repair
(#339). Either make it true or delete the claim.

### F8 (P3) — Sampling is unreproducible
Every `Distribution::sample(&self)` creates an internal `rand::rng()`; there
is no rng injection or seeding path (`Prob::sample` DOES take `&mut Rng` —
inconsistent). A verification library whose statistical backend cannot be
seeded cannot reproduce its own verdicts.

### F9 (P3) — Prob semantics conflated on distribution samples
`Bernoulli::sample` labels value `false` with probability `p` (the TRUE
probability of `true`); `Normal`/`Exponential` samples are wrapped
`Prob::new(value)` = probability 1.0 (test comment: "Sampling is
deterministic"). The wrapper means different things per distribution.

### F10 (P3) — ensures_expected helper: brittle pattern, no statistical basis
`let #expression = value;` generates invalid Rust for non-ident expressions
(`let a + b = value`); the check is a fixed-threshold mean comparison with no
confidence semantics. Also `base10_parse::<f64>().unwrap_or(0.0)` can write a
false 0.0 into generated docs.

### F11 (P3) — SMT float literals are inexact / can be syntactically invalid
`format!("{}", value)` emits shortest-roundtrip decimals with two genuine
defects (R1-corrected; the draft's "scientific notation" claim was false —
Display prints `1e20_f64` as the full integer `100000000000000000000`):
(1) exactness — 0.1 prints "0.1", an exact SMT decimal that is a DIFFERENT
number than the Rust f64 0.1000000000000000055…; (2) literal typing —
integers ≥ 1 print WITHOUT a decimal point, which SMT-LIB2 parses as Sort
Int, type-mismatching Real declarations (e.g. `(= bound 100000000000000000000)`
against a Real variable). Sound emission needs exact rational form
(`/ numerator denominator`) — never Display roundtripping.

### F12 (P3) — why3 backend is a deprecated string stub
Returns a placeholder string; `#[deprecated]` since 0.19.0. Disposition
decision needed (keep-deprecated vs remove; breaking changes acceptable).

## 3. Work units

- **U1 — Verification semantics re-orientation (F1).** Decide the semantic
  frame (D1), then fix: the verifier API, the macro-generated helpers, the
  SMT bridge (including predicate injection), and **the tests that codify the
  inversion**. TDD anchor cases: P(cond)=0.99 with bound 0.95 → Verified;
  P(cond)=0.02 with bound 0.95 → Violated; boundary behavior Inconclusive.
- **U2 — Solver-validated SMT emission (F2, F11).** Emit only QF_NRA-valid
  scripts (D2 strategy); tests invoke z3 (4.15.3, from the C3 install) on
  generated scripts and assert the parsed verdict — both directions
  (obligation that holds → unsat; obligation that fails → sat). R1-added —
  per-builder GUARANTEE CLASSES, stated in the module docs and enforced by
  per-builder controls: (a) `hoeffding_obligation` is fully
  solver-checkable (all variables constrained to literals; negated goal;
  unsat ⇔ the bound holds) — control pair: parameters where the bound holds
  (unsat) and parameters where it fails (sat); (b) `expected_value_obligation`
  declares `sample_mean` but never constrains it — as emitted it is NOT
  checkable; either constrain it (caller-supplied observed mean enclosure)
  or document it as a documentation artifact; (c) `pre/post obligation`s
  leave `p_condition` unconstrained except [0,1] and the negated goal, so
  unsat ⇔ bound > 1 — VACUOUS as checkable proofs; they are structured
  documentation unless the caller adds system assertions; the builders take
  an optional `system_assertions` parameter (richer substrate) so a
  meaningful control pair exists: same obligation with and without
  caller-supplied probability enclosures → sat vs unsat. Tests pin each
  builder's class with its control pair.
- **U3 — Statistical corrections (F3, F4, F5, F6).** Invert the crate's own
  `hoeffding_bound` for the epsilon; z-score handling per D3; chernoff
  validity domain enforced + documented; constructor asserts with
  fail-fast tests.
- **U4 — no_std truth (F7).** Repair to compile (alloc-gated contracts) or
  remove the claim — D4.
- **U5 — Reproducibility & sample semantics (F8, F9, F10).** rng-injection
  API alignment (D5) + documented Prob semantics on samples. F10 acceptance
  criteria (R1-added): (i) the generated expected-value helper must COMPILE
  for non-ident expressions — bind the call result to a fresh identifier
  (`let result = #fn_call;`) and evaluate `#expression` in scope (no
  `let <expr> = value` pattern-binding); (ii) the mean check gains explicit
  semantics: either a documented smoke check (fixed threshold, labeled as
  such) or a confidence-interval verdict — implementer documents which and
  the generated doc string matches; (iii) `unwrap_or(0.0)` in generated doc
  strings is removed — parse failures surface at macro expansion, not as a
  silent 0.0 in documentation.
- **U6 — Disposition of why3 (F12) per D6.**

Out of scope: the `geometric` feature (TODO already), new distributions,
karpal-verify's own re-export semantics (consumer impact handled in §5).

## 4. Decision points (operator)

- **D1 — Verification semantic frame.** (a) Hold-probability frame:
  `verify_probability_at_least(pred, 1-δ)`; helpers generated for that; docs
  say "holds with P ≥ p". (b) Violation frame: keep bound = violation
  probability δ, but verify P(¬cond) ≤ δ (what today's attribute docs
  describe). (c) Both: two explicit APIs, macros take a named form.
  Recommendation: (c) — richer substrate, no silent reinterpretation.
- **D2 — SMT validity strategy (R1-P1 amended).** Host-side f64 evaluation
  is NOT sound for real-valued proofs: 2·exp(−2000) rounds to 0.0 in f64
  while the true value is ≈ 5.153e−869 > 0; rationalizing that zero could
  prove a false bound. Sound options: (a) **Certified enclosures** — compute
  directed rational lower/upper bounds for transcendental terms (interval
  arithmetic with outward rounding), assert the conservative branch; the
  emitted script then proves a WEAKER but TRUE bound. (b) Symbolic structure
  with rational interval enclosures bracketing exp in-script. (c) Both:
  default (a) certified-enclosure emission, optional symbolic mode (b),
  plus an explicitly labeled "rounded computation" convenience mode that
  documents it is NOT a proof. Recommendation: (c) as amended — operator
  approved (c); the default inside (c) changes from roundtrip-decimal to
  certified enclosure (soundness amendment, flagged for operator).
- **D3 — z-scores.** (a) Exact table for {0.90, 0.95, 0.98, 0.99, 0.995,
  0.999} + panic/assert outside the table (explicit contract). (b) Proper
  inverse-normal approximation with documented error bound (Acklam-class).
  Recommendation: (b) with (a)'s exactness for tabulated levels.
- **D4 — no_std.** (a) Repair honestly (alloc feature for String-bearing
  types; format! usages moved/gated; CI check both feature states). (b)
  Delete the cfg_attr claim (std-only crate). Recommendation: (a) — matches
  the amari-core no-std posture.
- **D5 — rng injection scope.** (a) Full API alignment this package
  (`sample_with_rng` everywhere + seeded `MonteCarloVerifier`). (b) Document
  limitation, defer. Recommendation: (a) — it is the crate's reproducibility
  story and the code volume is small.
- **D6 — why3 stub.** (a) Remove in 0.25.0 (deprecated since 0.19, breaking
  OK). (b) Keep deprecated one more cycle. Recommendation: (a).

## 5. Consumer impact

karpal-verify (feature `amari`): re-exports the three macros (semantics
change under U1 propagates — its tests will surface any encoded inversion
there too), wraps all four obligation builders (U2 changes emitted scripts),
and — R1-corrected — `verify_rare_event` passes VIOLATION predicates
directly to the verifier, i.e. the one call-site whose shape is correct
under the CURRENT inverted API (violation-predicate + P(pred) ≤ δ is the
right form for violation bounds). Under D1(c) this maps to
`verify_violation_bound`; U1 must include BEHAVIORAL MIGRATION TESTS in the
consumer surface: an always-false violation predicate must verify, an
always-true violation predicate must fail, pinning the frame before and
after the re-orientation. Per doctrine, compatibility is not a constraint;
changes disclosed with migration notes in the crate CHANGELOG entry.

## 6. Verification matrix (implementation gate)

1. `cargo test -p amari-flynn -p amari-flynn-macros` (default): all green,
   including the re-orientation anchor tests (U1) and solver-validated SMT
   tests (U2).
2. `cargo test -p amari-flynn --no-default-features`: compiles and green (or
   claim removed per D4).
3. Clippy `--all-targets -D warnings`, fmt, scoped to both crates.
4. Solver round-trip: z3 parses EVERY generated script shape; known-sat and
   known-unsat controls.
5. Statistical oracle tests: epsilon equals the inverted own-hoeffding
   formula; z-scores within documented error of the true quantiles
   (spot-check against known values).
6. Doc build (`cargo doc`) with no broken intra-doc links.
7. Consumer smoke: karpal-verify `amari` feature builds against the repaired
   flynn (workspace path override).
