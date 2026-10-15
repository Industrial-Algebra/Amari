# M-REP-A evidence — claim repair (2026-10-15)

Approved matrix: `docs/plans/2026-10-09-amari-core-verification-matrix.md`
§8.1 D-B + §10 M-REP split. Scope: MATRIX-02 dot-contract repair, MATRIX-01
proof-implying wording repair. Nothing else (M-REP-B/C untouched).

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

### `.github/workflows/mathematical-correctness.yml` (wording only; all `run:`
commands, filters, conditionals and exit codes unchanged)

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
| :59/:60 | `# Count verification tests` / `VERIFICATION_COUNT` | `# Count files carrying formal-annotation markers` / `ANNOTATION_COUNT` |
| :63/:67 | `Verification tests found: …` / `verification=…` | `Files with formal annotations found: …` / `annotation=…` |
| summary :73 | `**Verification Tests**: …` | `**Files with Formal Annotations**: …` |
| badge file/label/message | `verification-report.json` / `"verification"` / `"N verified"` | `annotation-report.json` / `"formal annotations"` / `"N files with formal annotations"` |
| test badge message | `"N passing"` | `"N test functions found"` |
| artifact path | `verification-report.json` | `annotation-report.json` |
| PR comment | `verificationCount`, `**Verification Tests**`, `✅ All test counts verified!` | `annotationCount`, `**Files with Formal Annotations**`, `✅ Test and annotation counts reported.` |

Counts, arithmetic, filters, `test-count` output, badges shape and exit
semantics are unchanged.

### Found but intentionally NOT repaired

- `cargo test … verified_contracts` and `cargo test … geometric_axioms` /
  `semiring_axioms` filter arguments (M-REP-B scope).
- Public `Verified*` type names, the `formal-verification` Cargo feature name,
  and the `mathematical-correctness` workflow/job/cache identifiers (matrix
  MATRIX-08 required-check names; design choice, not a claim).
- `amari-core/src/verified_contracts.rs:47` `ensures(result >= T::zero())` on
  `verified_norm` — its `result >= T::zero()` contract was REMOVED in R1:
  falsified by NaN (`verified_norm([NaN,0]) == NaN`; `NaN >= 0` false);
  finiteness-guarded contract deferred to M-REP-C
  contract.
- **Out of scope, noted for a separate sweep:** `.github/workflows/status-badges.yml`
  still carries active `VERIFICATION_TESTS` labels and a `"N verified"` badge
  message. It is a third workflow, outside the two dispatched files.

## Verification — pinned recipe

`RUSTC`/`RUSTDOC` = stable via `rustup which`, `RUSTC_WRAPPER=''`,
`RUSTC_WORKSPACE_WRAPPER=''`, `RUSTFLAGS='-C target-cpu=x86-64 -C target-feature=-avx2'`,
`CARGO_TARGET_DIR=$PWD/target/mrepa`, `--locked`, stable Cargo (`cargo +stable`;
the toolchain file pins nightly and nightly Cargo/clippy-driver forwards `-Z`
flags that stable rustc rejects).

| Gate | Result |
|---|---|
| `cargo +stable test -p amari-core --locked` | **489 passed / 17 suites**, 0 failed, 0 ignored (develop baseline) |
| `cargo +stable test -p amari-core --features formal-verification --locked` | **493 passed / 17 suites**, 0 failed, 0 ignored |
| `cargo +stable clippy -p amari-core --all-targets --locked -- -D warnings` | exit 0, **0 warnings** |
| `cargo +stable clippy -p amari-core --all-targets --features formal-verification --locked -- -D warnings` | exit 0, **0 warnings** |
| `cargo +stable fmt --all --check` | exit 0 |
| `cargo +stable doc -p amari-core --no-deps --locked` (`RUSTDOCFLAGS=-D warnings`) | exit 0 |
| `cargo +stable check --workspace --locked` | exit 0 |

Logs: `amari-core-default-test.log`, `amari-core-formal-test.log`,
`amari-core-clippy-default.log`, `amari-core-clippy-formal.log`, `fmt-check.log`,
`amari-core-rustdoc.log`, `workspace-check.log`, `recipe-env.txt`. Full source
diff: `mrep-a.diff`. Red inventory: `../red/`.
