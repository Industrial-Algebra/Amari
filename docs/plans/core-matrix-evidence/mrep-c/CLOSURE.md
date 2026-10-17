# M-REP-C — Closure Record

**Merged:** PR #311 → develop `6a80128` (2026-10-17T01:24:54Z); branch tip
`6ab05d3` (catalog regen post-#313). Superseded on develop by #312
(`dedd1f6`, rewrite Task 34, with its own correct catalog regen `ff4a161`).

## Loop record (4 rounds, cap 12; convergence = one round with 0 P1/P2)

| Round | Verdict | Finding → remediation |
|---|---|---|
| R1 | 4 P1 | Operation-side finiteness holes: `verified_self_dot` NaN via `[MAX]+[-MAX]` add-chain; `scalar` contract contradicting its own `Err` path; `verified_scalar_mult` panic on `2.0×MAX` overflow; `compose` zero-normalization manufacturing NaN rotors → all made fallible (`Result`), contracts success-conditional, reachable unwraps 6→0. Commit `5213f0b`. |
| R2 | 1 P1 / 1 P2 / 1 P3 | Naive sum-of-squares in `compose` underflowed on subnormal cancellation (exact fixture `a=[s,0,0,0,0,s,0,0]∘b=[s,0,0,t,0,-s,0,0]`, only coeffs 3 & 6 nonzero at `1.626e-162`; naive norm `3.14e-162` → `Ok(norm 0.7317)` outside `[0.9999,1.0001]`); rustdoc promised an unusable revalidation route; deletion-line host paths → scale-first normalization + revalidation through `VerifiedRotor::new`; construction-only-guarantee docs; scrub. Commit `a0eddd8`. |
| R3 | 0 P1 / 2 P2 | The R2 battery pinned a same-theme but non-cancelling fixture (passed against broken code); revalidation route still not externally expressible (`new` takes `Vec`, coefficients private) → reviewer's exact arrays pinned (pre-fix norm in the comment); keep-the-`Vec` honest doc. Commit `45a2632`. |
| R4 | **CONVERGED (0/0/0)** | Corrected battery proven failing (`0.7317`) against the broken implementation, holding (`1.0`) at HEAD; all four R1 counterexamples still `Err`; gates green. |

**Method note (2nd instance of the lesson):** regression batteries pin the
reviewer's *exact* counterexample arrays, not a plausible same-theme fixture —
R3's battery would have passed against the broken `compose`.

## Final gate state (branch tip, stable 1.98.0)

amari-core 516/18 default, 526/18 formal (battery +1); clippy `-D warnings`
clean both configs; fmt clean; rustdoc clean; workspace `--all-targets`
clean; named suites `geometric_axioms` 6, `verified_contracts` 4; recursive
host-path search 0. Full inline loop record on PR #311 (4 review posts).

## Incident record — the red CI round that was not ours (2026-10-16)

#311's checks went 15/16 red at the *resolution* stage (2–23 s per job,
nothing compiled). Diagnosis: zip's entire 8.x line yanked from crates.io
(no advisories — deliberate 8.x retirement); `candle-core 0.11.0` requires
`zip ^8.6.0` = permanently unsatisfiable; five workflows resolve
`--all-features` and this repo does not commit `Cargo.lock`. Reproduced on
plain develop — **diff-independent**. Amari's candle was pre-declared (#242)
with zero `src/` imports; the only usage was the dependency-smoke test
itself. Resolution: **PR #313** removed the unused deps (merged `34effae`),
healing repo CI; upstream fix huggingface/candle#4041 remains unreleased
(quarterly cadence).

Aftermath, same day: merging develop into this branch without regenerating
the catalog tripped M-REP-B's drift gate (`generated_equals_checked_in`:
drift in `amari-core.json`, `amari-rewrite.json`, `manifest.json`) — the
gate's first live catch of a protocol miss. Fixed per protocol at `6ab05d3`
(28 crates, 11,158 items, hash `36aa8ed1`). Strategic outcome: the
**Katakuri** project (Shandora workspace) — IA-owned non-tensor model
ecosystem over Zunesha — plus the Borsalino tensor-extension handover;
see those repos' handover documents dated 2026-10-16.

## Standing residuals (carried to M-REP-D / pre-1.0, not blockers)

C3/C4 proof obligations undischarged by design (now gated on sound
contracts); unchecked-arithmetic overflow documented as
construction-scoped; no post-hoc revalidation API (documented honestly);
workspace `--all-features` local clippy blocked by pre-existing z3-sys
cmake environment issue; certified-phantom layer deferred to the future
pre-1.0 pass (Karpal proof/verify candidate).
