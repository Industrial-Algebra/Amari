# amari-flynn audit remediation — implementation evidence (U1–U6)

Branch fix/flynn-impl, contract PR #356 (D1(c), D2(c)-amended, D3(b), D4(a),
D5(a), D6(a)). Toolchain 1.98.0; z3 4.15.3 (~/.local/share/creusot/bin).

## U1 — verification direction re-orientation
- MonteCarloVerifier: verify_probability_at_least (hold frame) +
  verify_violation_bound (violation frame) + seeded _with_rng variants;
  old ambiguous verify_probability_bound REMOVED (F1).
- Anchors: P=0.99 vs 0.95 → Verified; P=0.02 vs 0.95 → Violated;
  violation-frame migration pair (always-false → Verified, always-true →
  Violated). macro_tests rewritten from the codified inversion
  ("correctly reports Violated") to hold-frame anchors.
- SMT bridge: hold frame + predicate injection; concentration path is a
  documented self-check.

## U2 — solver-validated SMT emission
- New backend::rational: exact f64→rational (bit decomposition, reduced),
  schoolbook bigint, CERTIFIED e^{-x} enclosures (alternating Taylor after
  scaling + interval squaring + outward rounding at 42 significant digits,
  shift derived from the NUMERATOR so the digit gap survives — tiny values
  keep strictly-positive lower bounds, the 2*exp(-2000) underflow trap).
- QF_NRA scripts contain no exp; literals are exact rationals; per-builder
  guarantee classes documented (hoeffding checkable; expected_value needs
  observe_mean; pre/post vacuous without add_system_assertion — unsat iff
  bound <= 0, z3-verified).
- z3 suite (feature z3-tests): 51/51 incl. both-direction controls, the
  loose-enclosure straddle tri-state, boundary characterizations, and the
  refutation-reading pair (system P=0.9 vs required 0.95 → sat;
  P=0.99 → unsat).

## U3 — statistical corrections
- hoeffding_epsilon: exact inversion of own hoeffding_bound
  (sqrt(ln(2/delta)/(2n))); property test binds bound<=delta tightly.
  Historical constant was 1.802x wider (0.0774 vs 0.0429 at n=1000).
- z-scores: Acklam inverse-normal + Halley refinement (libm erfc), exact
  table {90,95,98,99,995,999}‰; panics outside (0,1); the ±0.01 window
  misclassification and silent-1.960 fallback both gone. 0.999 interval
  strictly wider than 0.99 (anchor).
- chernoff_bound: domain asserted (delta in (0,1], mu >= 0), mu documented
  as the sum's expectation.
- Constructor invariants: Uniform min<max at construction; verifier
  samples >= 1, delta in (0,1].

## U4 — no_std made true
- Features: std = ["alloc"], alloc = []. RareEvent gated on alloc
  (alloc::string::String under no_std+alloc); distributions + backend::smt
  gated on std (thread-rng / format!+fs); prob/monte_carlo/statistical are
  no_std-clean (core + libm).
- cargo check green in all three states; tests green under
  --no-default-features (16/0 incl. doctest cfg-gating).

## U5 — reproducibility & sample semantics
- sample_with_rng on all four distributions (seeded reproducibility
  test: same seed → bit-identical draws); Prob labels: Bernoulli outcome
  probability (p / 1-p), Uniform outcome mass 1/n, Normal/Exponential
  unit weight documented as density-not-mass.
- ensures_expected helper: fresh-ident binding (no `let <expr> = value;`
  pattern), documented SMOKE-CHECK semantics, expansion errors instead of
  unwrap_or(0.0).

## U6 — why3 removed (D6(a))
- backend::why3 deleted; lib docs updated; CHANGELOG note in PR body.

## Verification matrix (final)
| Gate | Result |
|---|---|
| fmt both crates | clean |
| clippy both crates --all-targets -D warnings | clean |
| tests default + z3-tests | 55/0 lib, 16/0 macro_tests, suites green |
| tests --no-default-features | 16/0 |
| check --no-default-features --features alloc | clean |
| cargo doc | builds |
| consumer smoke | karpal-verify --features amari: 71/0 with the
  one-line D1(c) migration (verify_violation_bound); diff in PR body |

## Implementation war stories (for the review)
1. Interval squaring without precision management explodes digit counts
   (2^13 growth) — solved with outward rounding; first version rounded
   num/den INDEPENDENTLY to 42 digits, corrupting ratios by 10x when digit
   counts differed (probe: [3.6788, 0.3679]); second version shared the
   shift but zeroed tiny lower bounds; final: shift from the numerator,
   applied to both (gap invariant, lo > 0 always).
2. The vacuity witness as an ASSERTION changed the formula's
   satisfiability (unsat at every bound) — moved to comments.
3. Refutation-reading inversion in my own z3 test expectation (sat/unsat
   swapped for the violating system) — caught by running z3.
4. Parallel z3 tests clobbered a shared scratch file — unique per-call
   names (the #354 lesson, third instance).
