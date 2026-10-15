# M-REP-A evidence — claim repair (2026-10-15)

Approved matrix: `docs/plans/2026-10-09-amari-core-verification-matrix.md`
§8.1 D-B + §10 M-REP split. Scope: MATRIX-02 dot-contract repair, MATRIX-01
proof-implying wording repair, and the round 1–4 review remediations. Nothing
else (M-REP-B/C untouched).

## Final cumulative state

The cumulative change edits **four** workflows (all wording/label only; no
`run:` command, filter, schedule, or exit code changed):

| Workflow | Change |
|---|---|
| `.github/workflows/mathematical-correctness.yml` | proof-implying step/report wording → test-suite/instance wording |
| `.github/workflows/test-status.yml` | producer/consumer output label → `annotation`; per-test-function message |
| `.github/workflows/status-badges.yml` | comments/stdout/summary/badge template → annotation-inventory language |
| `.github/workflows/parallel-verification.yml` | report wording → test-suite language |

All four describe the same kind of value: a count of `#[test]` functions in
files carrying formal-annotation markers — **not** a file count and **not** a
count of proved statements. The message form “N test functions in
annotation-marker files” is used by both producer workflows.

**Scope caveat:** `test-status.yml` and `status-badges.yml` use different
marker/selection mechanics (the status-badges fallback matches only the
`formal_verification` marker). Identical message *semantics* does not mean
identical selected sets; selection/count mechanics are M-REP-B and are not
claimed identical here.

Source/doc changes in the cumulative diff: `amari-core/src/verified.rs`
(MATRIX-02 dot contract removal + rotor/verified docs),
`amari-core/src/verified_contracts.rs`, `amari-core/src/verified_laws.rs`,
`amari-core/src/comprehensive_tests.rs`, `amari-core/README.md`,
`docs/technical/MATHEMATICAL_RIGOR.md`,
`docs/technical/TEST_ENFORCEMENT.md`, `docs/development/CI_SETUP.md`.

## MATRIX-02 — unsound dot contract removed

`amari-core/src/verified.rs`, `Vector::dot`:

- Removed `#[cfg_attr(feature = "formal-verification", ensures(result >= T::zero()))]`.
  The contract is unsound for mixed vectors (`dot([1], [-1]) == -1`) and for
  indefinite signatures (the type is generic over all `P, Q, R`).
- Replaced with a rustdoc note recording the 2026-10-15 removal and assigning
  any sound self-dot-only contract for definite signatures to M-REP-C.
- **No new contract was added.** The change is annotation-only: under default
  compilation the `cfg_attr` predicate is false, so the attribute expands to
  nothing; only a doc comment is added. Ordinary compiled behavior is therefore
  unchanged and the default suite reproduces the develop baseline 489/17.

## MATRIX-01 — proof-implying wording → test/execution claims

### `.github/workflows/mathematical-correctness.yml` (wording only)

Note: the dispatch's line numbers were off by one in this checkout; the strings
below are the actual repaired occurrences.

| Location | Before | After |
|---|---|---|
| :64 | `✅ Core mathematical properties verified` | `✅ Core mathematical property tests passed` |
| :68/:70/:72 | `Run formal verification tests` / `Running formal verification tests...` / `✅ Formal verification contracts validated` | `Run formal-verification feature tests` / `Running formal-verification feature tests...` / `✅ Formal-annotation compatibility tests passed` |
| :80 | `✅ Documentation examples verified` | `✅ Documentation examples (doctests) passed` |
| :82/:84/:87 | `Verify geometric algebra axioms` / `Verifying geometric algebra axioms...` / `✅ All algebraic axioms satisfied` | `Run geometric algebra axiom tests` / `Running geometric algebra axiom tests...` / `✅ Algebraic test suites passed` |
| :89/:91/:94 | `Validate phantom type safety` / `Validating phantom type safety...` / `✅ Type safety constraints verified` | `Run phantom type-safety tests` / `Running phantom type-safety tests...` / `✅ Type-safety constraint tests passed` |
| :100/:104 | `VERIFICATION_TESTS` / `echo "  Verification tests: …"` | `FORMAL_FEATURE_TESTS` / `echo "  Formal-feature tests: …"` |
| :105 | `echo "  Mathematical rigor: 100%"` | `echo "  Mathematical test suites: executed"` |
| :132 | `✅ Workspace integration correctness confirmed` | `✅ Workspace integration tests passed` |
| :216 | `✅ Discovery integration correctness confirmed` | `✅ Discovery integration tests passed` |
| :270 | `❌ Mathematical integrity compromised!` | `❌ Mathematical-correctness test suites failed!` |
| :274 | `✨ Mathematical correctness maintained ✨` | `Mathematical-correctness test suites green` |
| :275–277 | `All geometric algebra properties verified` / `All tropical semiring axioms confirmed` / `All phantom type constraints validated` | `Geometric-algebra test suites passed` / `Tropical semiring test suites passed` / `Phantom type constraint test suites passed` |

### `.github/workflows/test-status.yml` (source-count inventory language)

| Location | Before | After |
|---|---|---|
| job output (:23) | `verification-count: …outputs.verification` | `annotation-count: …outputs.annotation` |
| :59/:60 | `# Count verification tests` / `VERIFICATION_COUNT` | `# Count #[test] functions in annotation-marker files` / `ANNOTATION_COUNT` |
| :63/:67 | `Verification tests found: …` / `verification=…` | `Test functions in annotation-marker files: …` / `annotation=…` |
| summary :73 | `**Verification Tests**: …` | `**Test Functions in Annotation-Marker Files**: …` |
| badge file/label/message | `verification-report.json` / `"verification"` / `"N verified"` | `annotation-report.json` / `"formal annotations"` / `"N test functions in annotation-marker files"` |
| test badge message | `"N passing"` | `"N test functions found"` |
| artifact path | `verification-report.json` | `annotation-report.json` |
| PR comment | `verificationCount`, `**Verification Tests**`, `✅ All test counts verified!` | `annotationCount`, `**Test Functions in Annotation-Marker Files**`, `✅ Test and annotation counts reported.` |

### `.github/workflows/status-badges.yml` (R4 finding 3)

| Location | Before | After |
|---|---|---|
| :45 | `# Count verification tests` | `# Count test functions in annotation-marker files (formal-verification feature)` |
| :48 | `# Alternative verification test count` | `# Alternative count: #[test] functions in files mentioning the formal_verification marker` |
| :55 | `echo "Found $VERIFICATION_TESTS verification tests"` | `echo "Found $VERIFICATION_TESTS test functions in annotation-marker files"` |
| :75 | `# - name: Create verification badge` | `# - name: Create annotation-count badge` |
| :82 | `#     label: Formal Verification` | `#     label: Formal Annotations` |
| :100 | `echo "- Verification Tests: $VERIFICATION_TESTS"` | `echo "- Test functions in annotation-marker files: $VERIFICATION_TESTS"` |

The internal variable `VERIFICATION_TESTS` and the `formal-verification`
feature/marker identifier are retained (identifiers, not claims), matching the
test-status approach. The `.github/workflows/parallel-verification.yml` report
block was already repaired in round 1 (`Geometric-algebra suites: ✓`, etc.) and
is confirmed by the R4 index as repaired.

## Round 4 remediation (this pass)

### F1 (P1) — `amari-core/src/verified.rs` rotor docs (index F: 428,437,447,493,500)

Rewrote the `VerifiedRotor` struct/field/method docs to describe the **actual**
checks:
- even grade: every odd-grade coefficient must compare equal to zero;
- unit norm: `(norm - 1).abs() <= 1e-4` where `norm = sqrt(sum(c_i^2))`.

Removed “compile-time guarantees”, the phantom-as-proof phrasing, “verified at
runtime, would be proven with Creusot”, and the unconditional “guaranteed valid
/ unit-norm preserved” composition claims. Added the recorded non-finite gap:
`VerifiedRotor::new(scalar(NaN))` returns `Ok` because `NaN > tolerance` is
false; `compose` performs no runtime validity check and divides by the computed
norm. The `formal-verification` attributes are labelled intended specifications,
not discharged proofs. **Behavior unchanged** (non-finite/specification
redesign remains **M-REP-C**).

### F2 (P1) — current methodology pages (index F: MATHEMATICAL_RIGOR, TEST_ENFORCEMENT, CI_SETUP)

Judgment disclosed: I **rewrote** these pages rather than marking them
historical, because `docs/README.md` links them as current methodology.
The rewrite separates three things:

1. **What ordinary tests establish** — executed instances on specific inputs
   (no universal law).
2. **What annotations express** — intended contract specifications compiled
   away in ordinary builds; phantom types are compile-time type checks;
   `formal-verification` currently means annotation/test compatibility, not a
   prover run.
3. **What would constitute proof** — C3 toolchain conformance and C4 discharged
   obligations; not done here.

The associativity caveat is recorded in both pages: scalar `f64`
`a = b = 1e308`, `c = 0` gives `(a*b)*c = NaN` while `a*(b*c) = 0`.
`docs/development/CI_SETUP.md`'s “Mathematical Integrity Guarantee” section was
replaced with “What this setup does (and does not) establish”, and the badge
description now references the annotation-inventory semantics.
`amari-core/src/comprehensive_tests.rs:4` (index F) no longer calls ordinary
tests “part of formal verification”.

### F3 (P2) — status-badges + CI_SETUP labels

See the status-badges table above; `CI_SETUP.md:45–48` now describes the badge
as “Test functions in annotation-marker files … a source-inventory count, not a
count of proved statements”.

### F4 (P2) — final evidence and PR description

This SUMMARY is rewritten to the four-workflow final state and test-function
semantics, with the contradictory surviving clauses removed. The PR body is
refreshed in the same pass. The saved `mrep-a.diff` is now explicitly labelled
the **historical first-round snapshot**; the cumulative source diff is
`mrep-a-final.diff` (12 files: 4 workflows + 8 source/doc files).

### Other in-fence occurrences swept

Every index line marked **F** was repaired in this pass: `TEST_ENFORCEMENT.md`
and `MATHEMATICAL_RIGOR.md` (whole-page rewrite), `status-badges.yml`
(45,48,55,75,82,100), `verified.rs` (whole rotor doc block), `comprehensive_tests.rs:4`,
`SUMMARY.md` (whole-file rewrite), `CI_SETUP.md` (whole-page rewrite). Index
lines marked **H** are already-honest wording/identifiers; **D** lines are
historical/other-crate/prospective text preserved as-is per the reviewer’s
disposition.

## Found but intentionally NOT repaired (out of M-REP-A)

- `cargo test … verified_contracts` and `cargo test … geometric_axioms` /
  `semiring_axioms` filter arguments (M-REP-B scope).
- Public `Verified*` type names, the `formal-verification` Cargo feature name,
  and the `mathematical-correctness` workflow/job/cache identifiers (matrix
  MATRIX-08 required-check names; design choice, not a claim).
- `amari-core/src/verified_contracts.rs:47` retained coefficient-equality
  annotations fail on NaN/inf; finiteness-guarded contracts are **M-REP-C**.
- Filter vacuity / marker-set differences / no-run parsing: **M-REP-B**.
- Rotor non-finite specification/implementation redesign: **M-REP-C**.

## Verification — pinned recipe

`RUSTC`/`RUSTDOC` = stable via `rustup which`, `RUSTC_WRAPPER=''`,
`RUSTC_WORKSPACE_WRAPPER=''`, `RUSTFLAGS='-C target-cpu=x86-64 -C target-feature=-avx2'`,
`CARGO_TARGET_DIR=$PWD/target/mrepa`, `--locked`, stable Cargo (`cargo +stable`;
the toolchain file pins nightly and nightly Cargo/clippy-driver forwards `-Z`
flags that stable rustc rejects).

| Gate | Result |
|---|---|
| `cargo +stable test -p amari-core --locked` | **489 passed / 17 suites**, exit 0, 0 failed, 0 ignored |
| `cargo +stable test -p amari-core --features formal-verification --locked` | **493 passed / 17 suites**, exit 0, 0 failed, 0 ignored |
| `cargo +stable clippy -p amari-core --all-targets --locked -- -D warnings` | exit 0, 0 warnings |
| `cargo +stable clippy -p amari-core --all-targets --features formal-verification --locked -- -D warnings` | exit 0, 0 warnings |
| `cargo +stable fmt --all --check` | exit 0 |
| `RUSTDOCFLAGS='-D warnings' cargo +stable doc -p amari-core --no-deps --locked` | exit 0 |
| `cargo +stable check --workspace --locked` | exit 0 |
| YAML validity (`yaml.safe_load`) for all four edited workflows | OK |

Logs (round 4): `amari-core-default-test.log`, `amari-core-formal-test.log`,
`amari-core-clippy-default.log`, `amari-core-clippy-formal.log`,
`fmt-check.log`, `amari-core-rustdoc.log`, `workspace-check.log`,
`recipe-env.txt`.

Diff artifacts:
- `mrep-a.diff` — **historical first-round snapshot (R1, commit `be1371c`)**;
  covers only `mathematical-correctness.yml`, `test-status.yml`, `verified.rs`.
  Not the final diff.
- `mrep-a-final.diff` — cumulative source/doc diff vs `origin/develop`
  merge-base `80ebe9e` (4 workflows + 8 source/doc files; evidence directory and
  session state excluded).
