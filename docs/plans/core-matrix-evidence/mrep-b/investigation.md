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
