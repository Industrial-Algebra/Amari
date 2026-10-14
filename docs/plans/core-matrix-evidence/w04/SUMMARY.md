# CORE-W04 — rotor domains, duality, grade-type enforcement (evidence summary)

Contract: docs/plans/2026-10-13-amari-core-w04-rotor-duality-contract.md (revision 9,
8-round independent review: R1 5P1/9P2 -> R8 0P1/0P2) + consumer-impact companion
(revision 9, 8-round review; compilation-honest per-operation disclosure).
Operator-approved 2026-10-13 after explicit hold-and-discuss + external-consumer
framing (33k downloads, ^0.24.1 pins).

Branch: fix/core-w04-rotor-duality from develop 50e9b46. Lock eb2442dc.
Red: 11 findings reproduced (findings.log); baseline 222 lib tests.

## Stages (all pinned-recipe, --locked, archived stage-*.log)
- S1 errors/privacy/Result: error enums; mv private + escapes + TryFrom; four
  fallback deletions; uniform R†/certified-N inverse; N-gates; consumer migration.
  223 lib / 397 total.
- S2 log/powers (+no_std repair of pre-existing W03 breakage): §2 a–d exact —
  scalar-first branches; exact B² classification; element-log scales; certified-N
  hyperbolic (exact-rational q, δ_num/N, ln1p/decomposition branches, no β/w);
  range-safe intermediates; power_integer routing + output gates. 231 lib / 420.
- S3 slerp/from_vectors/reflect: coefficient-dot sign rule + exact endpoints;
  exact 2×2-minor proportionality direction test; candidate trichotomy
  (NullCandidate/SheetChange); definite-2D-only antipodal resolution (indefinite
  2D ±e12 acts as identity — derived by implementer, Err(Antipodal)); grade-
  involution hyperplane reflect + reflect_line. 234 lib / 444.
- S4 duals/components: formal_complement (all signatures); metric hodge_dual
  with DegeneratePseudoscalar; generic typed dual deleted (compile_fail doctest
  pin); concrete 3D typed duals; lexicographic component map (relativistic
  boost-bivector fix fallout); e_ij strict. 234 lib / 466.

## Final matrix (final-*.log)
- amari-core: 466 tests / 17 suites on stable AND nightly, identical
  inventories; clippy 0 both; workspace all-targets 0; no-default 0; fmt clean.
  Test lineage: 385 (W03) -> 466.
- Catalog dfd35092…, 11,134 items — only amari-core.json + manifest.json
  changed (CAT1 isolation holding).
- Workspace clippy verified green directly (the S4 report's tropical flag was
  a stale-context artifact).
- Evidence paths scrubbed from creation ($AMARI/$HOME placeholders).

## Disclosed deviations (all in stage summaries)
S2: ln_ratio combined form (witness rapidity correctly rounded); binary gate via
exact scalar-part sign (multi-plane rounding); from_bivector non-simple ->
Degenerate (series N non-scalar under rounding); mixed-component fixture in
unit test (1-ulp conditioning); m+1=6 ulp budget; odd grades -> Uncertified
superset; integral-power fallback only on Uncertified; N-certified-before-
normalize ordering. S3: indefinite-2D antipodal excluded (e12 identity);
uncertifiable square -> NotNormalizable; C=0 path retained (proven implied).
S4: Bivector get/Index generic via general map ([3,5,6] literal removed);
from_components generic-delegating (4D third arg e14, documented; relativistic
boost map corrected); compile_fail doctest + grep (no trybuild); enumerative
.expect (all non-degenerate); audit-repro stand-alone crate untouched.

## Status
IMPLEMENTATION COMPLETE AND STAGED. PAUSED BEFORE THE IMPLEMENTATION REVIEW
LOOP per operator instruction 2026-10-13 ("do not run a review loop until
further notice"). Not committed; awaiting go/no-go.
