# M-REP-B per-site investigation and disposition (suite integrity)

Date: 2026-10-15
Approved matrix: `docs/plans/2026-10-09-amari-core-verification-matrix.md`
§3 "Named-suite filter integrity" (loop-2 F2) + §10 M-REP-B split (MATRIX-04).
Base: `develop` `e8559b7`; lock `eb2442dc`; worktree `core-mrep-b`; target
`target/mrepb`. Pinned recipe in `green/recipe-env.txt`.

Red evidence for the nine advertised filters is archived in
`red/filter-inventory.txt` (all nine exit 0 with `0 passed; N filtered out`) and
`red/invocation-sites.txt` (the nine invocation sites).

## Aggregate finding

**All nine advertised filters select zero tests as invoked.** Three root
causes, one per site class:

1. **Missing feature in the invocation.** `verified_contracts` (core) is
   `#[cfg(feature = "formal-verification")]`; `verified_contracts`/`semiring`
   (tropical) are `#[cfg(feature = "contracts")]`. The invocations passed
   neither feature, so the modules were not compiled.
2. **Tests that never existed.** `amari-core geometric_axioms` has never had a
   test function or module whose path contains `geometric_axioms`; the filter
   matched nothing in any of the 17 (then) targets. `amari-network
   verified_contracts` has **no `#[test]` functions at all**, even with
   `--features formal-verification`.
3. **Wrong filter string.** `amari-tropical semiring_axioms` names the public
   `semiring_axioms` module, which contains **no `#[test]` functions**; the real
   semiring-axiom tests are `verified_contracts::contract_tests::test_maxplus_semiring_contracts`
   and `...test_minplus_semiring_contracts`.

Because `cargo test <filter>` exits 0 when the filter matches nothing, each of
the nine sites was a silent fake-green. `scripts/verify-named-suite.sh` now
converts every surviving invocation into an executable gate (inventory + `passed
>= 1` + `ignored == 0`).

## Module gating (verified in source)

| Crate | Module | Gate | Tests |
|---|---|---|---|
| amari-core | `verified_contracts` | `#[cfg(feature = "formal-verification")]` (`src/lib.rs:74-75`) | 4 (`verified_contracts::contract_tests::*`) |
| amari-tropical | `verified_contracts` | `#[cfg(feature = "contracts")]` (`src/lib.rs:37-38`) | 5 (`verified_contracts::contract_tests::*`) |
| amari-tropical | `semiring_axioms` (inside `verified_contracts`) | `contracts` | 0 — module has no `#[test]` |
| amari-network | `verified_contracts` | `#[cfg(feature = "formal-verification")]` (`src/lib.rs:61-64`) | 0 — no `#[test]` functions in the file |

## Per-site disposition table (nine sites)

| # | Site | Filter as invoked | Why vacuous | Disposition |
|---|---|---|---|---|
| 1 | `.github/workflows/mathematical-correctness.yml:85` | `amari-core geometric_axioms` | no such tests exist (never did); `-S geometric_axioms -- '*.rs'` is empty across history | **new-suite** — `amari-core/tests/geometric_axioms.rs` adds 6 real public-API instance tests; invocation retained + assertion |
| 2 | `.github/workflows/mathematical-correctness.yml:86` | `amari-tropical semiring_axioms` | module has no `#[test]`; real axiom tests named `*_semiring_contracts`; module needs `contracts` | **fixed-invocation** — filter `semiring_contracts` + `--features contracts` + assertion |
| 3 | `.github/workflows/mathematical-correctness.yml:92` | `amari-tropical verified_contracts` | missing `--features contracts` | **fixed-invocation** — add `--features contracts` + assertion (5 tests) |
| 4 | `.github/workflows/mathematical-correctness.yml:93` | `amari-core verified_contracts` | missing `--features formal-verification` | **fixed-invocation** — add `--features formal-verification` + assertion (4 tests) |
| 5 | `.githooks/pre-commit:128` | `amari-core geometric_axioms` | same as #1 | **new-suite** + assertion |
| 6 | `.githooks/pre-commit:136` | `amari-tropical semiring_axioms` | same as #2 | **fixed-invocation** + assertion |
| 7 | `.githooks/pre-commit:144` | `amari-core verified_contracts` | same as #4 | **fixed-invocation** + assertion |
| 8 | `.githooks/pre-commit:149` | `amari-tropical verified_contracts` (TOLERATED, `2>/dev/null`) | missing `--features contracts`; tolerated so failure only echoed | **fixed-invocation** + assertion; now **BLOCKS** (deviation from the dispatch's "remove" hint — see note) |
| 9 | `.githooks/pre-commit:152` | `amari-network verified_contracts` (TOLERATED, `2>/dev/null`) | the module genuinely has **no `#[test]` functions**, even with `formal-verification` | **removed** (fake-green step deleted; in-hook comment + this record) |

### Note on site 8 (deviation from the dispatch hint)

The dispatch listed both hook tolerant sites (`amari-tropical` and
`amari-network verified_contracts`) for removal, on the premise that "tests
never existed". The per-site investigation disproves the premise for
`amari-tropical`: five real tests exist under `--features contracts`. Rule (i)
of the dispatch ("real tests exist but the invocation is wrong → fix the
invocation") therefore applies. The site is fixed and made blocking with the
same assertion as the other sites; no coverage is lost. Only site 9 is removed,
where the tests genuinely do not exist. The negative-control run in
`green/named-suite-assertions.log` records
`amari-network :: verified_contracts` selecting 0 tests (the reason for its
removal).

## Coverage-floor defect (hook rows 12-13)

Two independent defects, both repaired (hook and workflow):

1. **Failure shape.** `TOTAL_TESTS=$(... | grep -c ": test$" || echo "0")`
   appends a second `0` whenever `grep -c` exits nonzero, producing the
   two-line string `0\n0`. `[ "$TOTAL_TESTS" -lt 300 ]` then errors on the
   non-integer and the gate does **not** block — vacuous on failure.
2. **Wrong pattern in the hook.** The hook used `grep -c "test "` while
   `--list` emits `name: test` lines; the pattern matched **zero** lines on a
   successful gather too. So the hook's floor was vacuous **always**, not only
   on failure. (The workflow used the correct `: test$` pattern; only its
   failure shape was defective.)

Repair: `... | grep -c ": test$" || true`, then
`if [ -z "$TOTAL_TESTS" ]; then TOTAL_TESTS=0; fi`, keeping the ≥300 floor.
Evidence: `green/coverage-floor-repair.log` (old shape `$'0\n0'` → no block;
new shape `0` on failure → blocks; real gather `2075` → passes).

## Executable assertion

`scripts/verify-named-suite.sh` (new, shared by hook and workflow):

- lists the filtered tests (`-- --list`), requiring the selected set to have
  **exactly** the approved inventory size and to contain every approved name;
- runs the filtered tests and parses the `test result:` lines, requiring
  `passed >= 1` and `ignored == 0`;
- exits nonzero on a compile failure, a vacuous selection, a missing expected
  name, a test failure, or an unexpected ignored test.

Approved inventories (verbatim, as passed to the helper):

| Suite | Package | Features | Inventory |
|---|---|---|---|
| `geometric_axioms` | amari-core | — | `geometric_axioms::{additive_identity, associativity, basis_vector_signature, distributivity, multiplicative_identity, scalar_commutes}` |
| `semiring_contracts` | amari-tropical | `contracts` | `verified_contracts::contract_tests::{test_maxplus_semiring_contracts, test_minplus_semiring_contracts}` |
| `verified_contracts` | amari-core | `formal-verification` | `verified_contracts::contract_tests::{test_signature_verification, test_verified_addition_properties, test_verified_identity_laws, test_verified_scalar_multiplication}` |
| `verified_contracts` | amari-tropical | `contracts` | `verified_contracts::contract_tests::{test_matrix_contracts, test_maxplus_semiring_contracts, test_minplus_semiring_contracts, test_multivector_contracts, test_tropical_identities}` |

---

## Round-2 expansion — exhaustive live string-filter index (review R1 F3)

Date: 2026-10-15. Round-1 review HEAD: `601bb3b` (the base for this
remediation); worktree `core-mrep-b`; target `target/mrepb`.

Review R1 found that the nine-site record above is not the exhaustive
filter-invocation index the matrix requires. Enumerated with the reviewer's
command at `601bb3b`:

```bash
git grep -nE 'cargo( \+[^ ]+)? test' -- ':!docs' ':!*.md' ':!*.rs' ':!Cargo.lock'
```

Scope rule (matching the reviewer's round): a **string-filter site** is any
executable invocation that passes a positional test-name filter — either to
`cargo test` directly or through the `verify-named-suite.sh` wrapper (which
passes the filter on to cargo). Excluded: whole-suite runs with no positional
filter (`cargo test --workspace`, `cargo test --test integration`), target-only
`--test <name>` selections with no filter, and `-- --list` / `--no-run`
gathers (the latter are dispositioned under the coverage floor above).
Historical `docs/` and plan prose are not executable and are out of scope.

The full live index is **16 sites**: 8 wrapped in `verify-named-suite.sh`, 7 in
the legacy `run_all_tests.sh`, and 1 discovery generator (which this
remediation wires, making it a 9th wrapped site).

| # | Site at `601bb3b` | Filter | Class | Disposition |
|---|---|---|---|---|
| 1 | `.github/workflows/mathematical-correctness.yml:85` | `amari-core geometric_axioms` | wrapped | evidence gate — asserted inventory of 6 |
| 2 | `.github/workflows/mathematical-correctness.yml:92` | `amari-tropical semiring_contracts` (`contracts`) | wrapped | evidence gate — 2 |
| 3 | `.github/workflows/mathematical-correctness.yml:100` | `amari-tropical verified_contracts` (`contracts`) | wrapped | evidence gate — 5 |
| 4 | `.github/workflows/mathematical-correctness.yml:106` | `amari-core verified_contracts` (`formal-verification`) | wrapped | evidence gate — 4 |
| 5 | `.githooks/pre-commit:128` | `amari-core geometric_axioms` | wrapped | evidence gate — 6; blocks |
| 6 | `.githooks/pre-commit:142` | `amari-tropical semiring_contracts` (`contracts`) | wrapped | evidence gate — 2; blocks |
| 7 | `.githooks/pre-commit:152` | `amari-core verified_contracts` (`formal-verification`) | wrapped | evidence gate — 4; blocks |
| 8 | `.githooks/pre-commit:161` | `amari-tropical verified_contracts` (`contracts`) | wrapped | evidence gate — 5; blocks |
| 9 | `scripts/generate-discovery-wasm-surface.sh:59` | `amari-discovery generated_has_valid_schema_version` | evidence-bearing (CI + publish) | **wired by this remediation** — asserted inventory of 1 (was a `tail -5` zero-match pass-through) |
| 10 | `run_all_tests.sh:67` | `amari-core geometric_product_tests` | legacy runner | not-evidence-bearing (see note) |
| 11 | `run_all_tests.sh:72` | `amari-core product_tests` | legacy runner | not-evidence-bearing |
| 12 | `run_all_tests.sh:77` | `amari-core rotor_tests` | legacy runner | not-evidence-bearing |
| 13 | `run_all_tests.sh:82` | `amari-tropical tropical_tests` | legacy runner | not-evidence-bearing |
| 14 | `run_all_tests.sh:87` | `amari-dual dual_tests` | legacy runner | not-evidence-bearing |
| 15 | `run_all_tests.sh:92` | `amari-info-geom info_geom_tests` | legacy runner | not-evidence-bearing |
| 16 | `run_all_tests.sh:127` | `integration test_performance` (`--test integration`) | legacy runner | informational-count / out-of-fence (release-profile performance signal; not a correctness gate) |

### Note on the seven legacy runner sites (10–16)

`run_all_tests.sh` is a standalone developer-convenience runner. It is not
invoked by any CI workflow, and `scripts/verify-publish-test-scope.py:70-71`
explicitly forbids its use in publish (`run_all_tests.sh reintroduces
amari-gpu runtime tests`). Its filters are therefore **not evidence-bearing**:
they carry no approved inventory, no vacuity assertion, and are not relied upon
as verification evidence. Current matches (independently listed by review R1):
`geometric_product_tests`=14, `product_tests`=37, `rotor_tests`=29,
`tropical_tests`=6, `dual_tests`=4, `info_geom_tests`=4. If the runner is ever
wired into CI or publication, each filter must first be wrapped with
`verify-named-suite.sh` and an approved inventory. Site 16 is additionally
out-of-fence: it is a `--release` performance signal, not a correctness gate.

### Note on site 9 (discovery generator)

`scripts/generate-discovery-wasm-surface.sh` runs in CI
(`.github/workflows/ci.yml:230`) and at publication
(`.github/workflows/publish.yml:77`). Its previous verification step,
`cargo test -p amari-discovery --test catalog_wasm -- generated_has_valid_schema_version 2>&1 | tail -5`,
accepted a zero-match exit-0 run as green. It now calls
`verify-named-suite.sh amari-discovery generated_has_valid_schema_version
generated_has_valid_schema_version`, which asserts the one-test approved
inventory and blocks on a vacuous selection. Evidence:
`green/r1/named-suite-assertions.log`.

## Round-2 remediation — F1 / F2 (review R1)

### F1 — exact-inventory semantics (`scripts/verify-named-suite.sh`)

The round-1 helper compared a raw count plus a substring membership search
(`grep -qF "$name: test"`), so a prefixed rename
(`renamed::geometric_axioms::associativity`) satisfied the approved
`geometric_axioms::associativity`, and duplicate approved arguments were
accepted. The helper now:

1. rejects duplicate approved names at argument-parse time (usage error, exit 2);
2. normalizes both sides to sorted multisets of FULL test names (the `--list`
   records with the trailing `: test` stripped) and requires exact equality —
   no missing, no extra; a prefixed rename is now a mismatch.

Negative controls: `scripts/mrep-b-negative-batteries.sh` (rename mock →
exit 1; duplicate-args mock → exit 2). Evidence:
`green/r1/mrep-b-negative-batteries.log`.

### F2 — preserve cargo's exit status (hook + workflow)

The round-1 coverage gather piped `cargo ... | grep -c ... || true`, discarding
cargo's status; a partial listing followed by a failed target passed. Both the
hook (`.githooks/pre-commit`) and the workflow
(`.github/workflows/mathematical-correctness.yml`, both `TOTAL_TESTS` and
`FORMAL_FEATURE_TESTS`) now capture the listing and its exit status separately
(`OUT=$(cargo ...) || STATUS=$?`), fail closed on any nonzero status, and keep
the well-formed-count handling (`grep -c ": test$" || true` + empty guard)
only for `rc=0`. Negative control: 300 records then exit 101 now blocks.
Evidence: `green/r1/coverage-floor-repair-r1.log` and
`green/r1/mrep-b-negative-batteries.log`.

The R1 review also flagged that `docs/technical/TEST_ENFORCEMENT.md` claimed a
gather failure was normalized to `0`; that overclaim is corrected there to
state the status is captured separately and any nonzero status blocks before
the floor is evaluated.

### Exhaustiveness

`git grep` was re-run at the remediation HEAD; the 16-row index above is the
full live enumeration under the scope rule. No site is silently skipped.

The remediation adds `scripts/mrep-b-negative-batteries.sh`, an offline test
harness in which `cargo` is a shell-function mock. Its `cargo test ... --list`
lines are fixture code with no positional filter (and never invoke a real
toolchain), so they fall under the `--list`-gather exclusion and are not live
filter sites. Its `bash "$HELPER"` calls exercise the four suite variants as
assertions, not as additional invocation sites.
