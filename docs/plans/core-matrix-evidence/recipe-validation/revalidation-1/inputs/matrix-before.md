# CORE-D07 — Complete Extended Verification Matrix — Decision Draft

> **REQUIRED SUB-SKILL:** Use executing-plans and ia-moment only after the applicable matrix and bounded implementation contract are approved. This document defines gates; it is not authority to change algorithms, dependency contracts, workflows or release policy.

**Goal:** Make every feature, compiler, target, consumer and proof claim in the core remediation programme accountable to executable evidence.

**Architecture:** Keep ordinary compilation, runtime tests, static rejection, target portability, safety and deductive proofs separate. Enumerate isolated core configurations rather than relying on workspace feature unification. Define when each gate blocks a package and when it blocks 1.0 readiness; later gates are not waived or reported green by earlier package completion.

**Tech Stack:** Existing Cargo workspace, Rust 1.89.0 / 1.98.0 / pinned nightlies, host and target test runners, independent blade-word oracle, separately installed Creusot 0.8.0 and matched proof tools. No production dependency or toolchain-file change is approved by this draft.

---

## 1. Approved decisions and remaining approval

The maintainer selected through `ask_user` in this session:

1. **Define the complete extended matrix before remediation implementation.** Do not infer D07 approval from the merged audit PR.
2. **Retain existing portability promises.** Require Linux x86_64 baseline/AVX2 execution, WASM compilation plus Node integration, and a genuine `thumbv7em-none-eabihf` library check for the supported alloc-based core. Restore dependency/no_std portability as a separate prerequisite before 1.0; host import repairs alone do not close it.
3. **Stage actual proofs before 1.0 readiness.** Require honest formal-compatibility tests now; track proof-tooling/specification repair and the full phantom/formal audit as explicit pre-1.0 gates. No proof claim until nonempty obligations are discharged. W00/W01 may proceed once their complete applicable executable matrix is approved and green.

**Pending:** approval of the concrete rows, compiler versions, gate scheduling and ownership below. No exception or production implementation has been approved. D01–D06 remain unresolved. This record supplements, rather than rewrites, the historical decision register from PR #276.

## 2. Baseline and reproducibility

- Source: `ba1b7719f609e8f0f127d620ba6936cfffb61413`, `origin/develop`, merge of PR #276.
- Research branch/worktree: `chore/core-verification-matrix`, `.worktrees/core-matrix`.
- Source/manifests/toolchain/workflows relevant to core are identical to audit baseline `165d7ccbd6d1aa899babb2d0ae392fa15272e556`.
- Research root lock explicitly generated with `cargo +nightly generate-lockfile --offline`, then missing artifacts downloaded with `cargo +nightly fetch --locked`. Neither operation edited a tracked manifest. Subsequent probes used `--locked --offline`.
- Research lock SHA-256: `acb76177dfb6ad26413de8a56c8ac9a6f8c6f2ec866516e6f95ceb828b6159f5`; 375 packages. This is a **new dependency resolution**, not the historical audit lock. Do not claim source identity implies dependency identity.
- Installed research nightly: `rustc 1.100.0-nightly (787af2b8c 2026-08-25)`; manifest date `2026-08-26`; Cargo `1.100.0-nightly (e8cb624d5 2026-08-22)`.
- Installed stable: `rustc 1.98.0 (88d9e12ae 2026-08-18)`; manifest date `2026-08-20`; Cargo `1.98.0 (797e8a9bc 2026-08-05)`.
- Host: `x86_64-unknown-linux-gnu`, AMD Ryzen AI 9 HX 370; AVX2 available. Availability is not SIMD safety certification.
- Only the host target is installed; the MSRV, CI-pinned nightly and actual proof toolchain were not installed or executed in this research pass.
- Logs: `/tmp/amari-core-d07/` (27 baseline rows: 8 pass / 19 fail; 19 extended rows: 14 pass / 5 fail; dependency tree, formal counterexample and zero-test filter evidence); independent scrutiny: `/tmp/core-d07-scrutiny/`. Selected logs, the two probe inventories and the root lock snapshot are additionally preserved in [core-matrix-evidence/](core-matrix-evidence/). The absolute `/tmp` paths in the inventories identify original research logs; they are not portable links. Uncopied logs remain temporary. These are research evidence, not archived release artifacts.

For every candidate, record source SHA, full command, feature closure, target, compiler/Cargo/Clippy versions, environment flags, lock hash, raw log, exit status, executed counts and skipped/ignored inventories. Archive the exact ignored lock as a named evidence artifact. Use `--locked` thereafter; offline is optional. Never silently resolve again after a missing cache item or an MSRV failure.

Run core gates with `-p amari-core` alone in a workspace invocation (or a deliberately isolated consumer fixture), not alongside another package that re-enables defaults. Keep dependency-only host checks distinct from executable tests and target-only checks.

## 3. Complete feature inventory

`cargo metadata --no-deps` reports **13 selectable non-default feature names**:

| Name | Effective activation / role |
|---|---|
| `std` | Core std branches |
| `phantom-types` | Reachable `verified` module |
| `gf2` | GF(2) module and its tests |
| `high-precision` | Dashu dependency with its std feature; Dashu implementation when native is off |
| `native-precision` | Rug dependency and native implementation; takes precedence over Dashu |
| `full-precision` | Alias: high + native |
| `wasm-precision` | Alias: high; CPU-testable, not excluded with binding-crate audits |
| `serialize` | Enables implicit `serde` feature/dependency |
| `serde` | Implicit optional-dependency feature |
| `parallel` | Enables implicit `rayon` feature/dependency |
| `rayon` | Implicit optional-dependency feature |
| `formal-verification` | Enables `creusot-contracts` and phantom types; ordinary compilation is not proof |
| `creusot-contracts` | Implicit optional-dependency feature; does not itself enable formal core modules |

Default: `std,phantom-types,high-precision,gf2`. Also run `--all-features` explicitly.

There is **no implicit `rug` feature**: the `dep:rug` syntax suppresses it. Do not fabricate a matrix row for that name.

### 3.1 Exhaustive host configuration family H

Enumerate all 8,192 subsets of the 13 selectable names and compute manifest activation closure. At this baseline these produce **1,440 distinct named closed sets**. For source/implementation coverage they normalize to **320 capability configurations**:

- std: off/on (2)
- GF(2): off/on (2)
- verification/dependency state: none, contracts dependency only, phantom only, phantom + contracts dependency, formal (5)
- precision: neither, Dashu only, native only, both (4)
- Serde dependency: off/on (2)
- Rayon dependency: off/on (2)

`2 × 2 × 5 × 4 × 2 × 2 = 320`.

Canonical spellings use `serde` and `rayon` for dependency toggles, `high-precision`/`native-precision` for precision toggles, and the five verification states above. Every row uses `--no-default-features` plus its canonical feature list (omit `--features` for the empty set).

Deduplication is justified **only for the present source and pinned dependency activation**: named aliases do not select a distinct implementation predicate here. Save the map from every named closed set to its canonical row. Separately check/test all 13 singleton names both alone and added to `std`; run named default, all-feature, full-precision and wasm-precision rows. Compare their resolved dependency activation and expected public precision type with the canonical equivalents. If a new source predicate or dependency activation distinguishes an alias, expand H; never reuse the 320 count by assumption.

A representative generator for the canonical family is:

```python
from itertools import product

verification = [(), ('creusot-contracts',), ('phantom-types',),
                ('phantom-types', 'creusot-contracts'),
                ('formal-verification',)]
precision = [(), ('high-precision',), ('native-precision',),
             ('high-precision', 'native-precision')]
rows = []
for std, gf2, v, p, serde, rayon in product(
        (False, True), (False, True), verification, precision,
        (False, True), (False, True)):
    features = [*v, *p]
    features += ['std'] if std else []
    features += ['gf2'] if gf2 else []
    features += ['serde'] if serde else []
    features += ['rayon'] if rayon else []
    rows.append(sorted(features))
assert len(rows) == 320
assert len({tuple(row) for row in rows}) == 320
```

This is an enumeration specification, not a claim that all 320 configurations were executed during planning. The executor must add manifest-closure and dependency-activation validation, preserve failure exits, and archive the generated matrix. New serialization/parallel functionality is not authorized: current source has no corresponding feature-gated operation surface. Record capability coverage gaps; enabling a dependency alone is not proof of serialization/parallel behavior.

### 3.2 Host commands for every H row

Before any H command, force the baseline ISA rather than inheriting `target-cpu=native`, encoded flags or target overrides. Use a Bash wrapper such as:

```bash
HOST=x86_64-unknown-linux-gnu
baseline_cargo() {
    local compiler documenter
    compiler=$(rustup which --toolchain "$T" rustc) || return
    documenter=$(rustup which --toolchain "$T" rustdoc) || return
    env -u CARGO_ENCODED_RUSTFLAGS -u CARGO_ENCODED_RUSTDOCFLAGS \
        -u CARGO_BUILD_TARGET -u CARGO_BUILD_RUSTC -u CARGO_BUILD_RUSTDOC \
        -u RUSTC_WRAPPER -u RUSTC_WORKSPACE_WRAPPER \
        -u CARGO_BUILD_RUSTC_WRAPPER -u CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER \
        RUSTC="$compiler" RUSTDOC="$documenter" \
        RUSTFLAGS='-C target-cpu=x86-64 -C target-feature=-avx2' \
        RUSTDOCFLAGS='-D warnings -C target-cpu=x86-64 -C target-feature=-avx2' \
        CARGO_TARGET_DIR="$PWD/target/d07-baseline/$T" \
        cargo +"$T" --config 'build.rustc-wrapper=""' \
            --config 'build.rustc-workspace-wrapper=""' "$@"
}
```

Set T to an approved compiler. A nonempty explicit `RUSTFLAGS` overrides Cargo-config rustflags; removing the encoded variable prevents it taking precedence. Explicit `--target` below overrides target configuration. Wrappers are disabled for this evidence gate so they cannot inject unrecorded compiler options. Pin rustc and rustdoc to the selected rustup toolchain as shown; merely removing `RUSTC` would leave Cargo executable configuration, and leaving `RUSTDOC` would permit an inherited documenter override. Apply matching baseline flags to rustdoc; removing encoded rustdoc flags also preserves `-D warnings`. This recipe remains **unexecuted after these corrections**; the [bounded validation/review checkpoint](2026-10-09-amari-core-matrix-validation-checkpoint.md) defines how to validate it without beginning remediation.

For each T, archive `rustc +"$T" --target "$HOST" -C target-cpu=x86-64 -C target-feature=-avx2 --print cfg`; assert that AVX/AVX2/FMA are absent. Obtain and assert the effective Cargo target cfg through the wrapper with `baseline_cargo rustc --locked -p amari-core --lib --target "$HOST" F -- --print cfg`, and inspect an initial verbose Cargo/rustdoc invocation to verify the actual flags. Stop if the effective configuration differs; recording contamination after tests is not sufficient. This cfg step does not execute the library.

Then for each canonical F:

```sh
baseline_cargo check --locked -p amari-core F --target "$HOST" --all-targets
baseline_cargo test --locked -p amari-core F --target "$HOST"
baseline_cargo clippy --locked -p amari-core F --target "$HOST" --all-targets -- -D warnings
baseline_cargo doc --locked -p amari-core F --target "$HOST" --no-deps
```

Here `T` and `F` are placeholders substituted as argument lists, not literal command arguments. Use the same sanitized baseline environment for alias/default/MSRV/ordinary-nightly host rows and standalone regressions. Non-host targets require target-appropriate flags, not `target-cpu=x86-64`. Explicit AVX2 uses a separate artifact directory and separately validated feature requirements **only after its safety prerequisite**; never reuse the baseline wrapper with native flags appended. Full `cargo test` includes unit/integration/doctests and builds examples; all-target compilation/Clippy additionally covers benches without invoking benchmark timings. Feature-off host tests must compile: use explicit test imports/gates, not deletion of tests or enabling absent features to avoid a failure.

Counts vary by configuration: GF(2), phantom, formal compatibility and precision tests appear only when their modules are enabled. Require a reviewed, nonempty baseline manifest of executed test names per row. Disabled suites must be accounted for; ignored tests require named dispositions. Check the basic example in representative default and minimal std rows.

For W01 add debug/release exact dyadic arithmetic regressions, the full independent blade-word oracle, and both original CORE-02 probes. Unrelated historical red probes remain red and are reported by finding; they are not a failing production suite to waive.

## 4. Compiler matrix and package scheduling

| Compiler | Scope | Gate / owner |
|---|---|---|
| `1.98.0` (current researched stable) | Full H commands; alias/default/all-feature rows | W00/W01 local acceptance; session executes, core maintainer accountable |
| `nightly-2026-08-26` (research compiler manifest date) | Full H commands, including warning-denied Clippy | W00/W01 local acceptance; do not substitute floating nightly without recording change |
| `1.89.0` | Library compile checks across H; separately compile the default workspace, matching CI | W00/W01 acceptance and 1.0 floor; test/dev-dependency support is a separate row, not implied |
| `nightly-2026-07-13` | Default, native-precision and all-feature test/build compatibility; native workspace CI | W00/W01 CI compatibility; CI maintainer accountable |
| Rolling stable | All current repository PR workflows | Every PR; record the actual compiler from that run rather than claiming it is necessarily 1.98.0 |
| Creusot-matched `nightly-2025-11-13` | Dedicated proof fixture and approved verification target, not the ordinary whole-workspace gate | Pre-1.0 proof gate; see section 7 |

Install exact ordinary compiler aliases before executing the approved matrix and verify that `nightly-2026-08-26` matches the researched rustc hash. If it does not, stop and reconcile; the date alone is not verified equivalence. Do not edit the repository's toolchain file just to make local checks pass.

The MSRV promise covers library configurations; supporting dev dependencies on MSRV is not automatically asserted. `amari-core/Cargo.toml` currently omits `rust-version.workspace = true` although the workspace declares 1.89. Document/repair that metadata in a bounded follow-up. A lock that needs a newer compiler is a blocker to investigate, not permission to raise the MSRV or silently downgrade dependencies. Record any separately approved MSRV-compatible resolution with its own hash and compare graphs.

## 5. Target, hardware and safety matrix

| ID | Target / configurations | Evidence | Gate / accountable owner |
|---|---|---|---|
| P-HOST | `x86_64-unknown-linux-gnu`, baseline instruction set, full H | Compilation + runtime + docs + Clippy | W00/W01; core maintainer, executed by session |
| P-AVX2 | Same host, explicit `RUSTFLAGS='-C target-feature=+avx2'`, representative default/minimal/native/all-feature configurations | Compile, runtime SIMD inventory, independent arithmetic checks; compare scalar expectations | Pre-1.0, before accepting SIMD/performance claims; core maintainer / W08 unsafe-audit owner |
| P-WASM | `wasm32-unknown-unknown`, portable configurations (native precision absent), including default, minimal std, pure high precision, wasm alias, phantom, GF(2), serialization and formal-compatibility selections | Core target library checks; existing binding build + Node tests; inventory portable feature combinations and runner-reachable behavior | Existing CI rows every PR; complete portable configuration closure before 1.0; portability/CI maintainer |
| P-EMBED | `thumbv7em-none-eabihf`, no std, supported alloc-based minimal core, then independent GF(2) and phantom combinations | Actual target library compile + target dependency tree with no target-side std; allocator/link/board requirements documented | Pre-1.0 portability prerequisite; portability maintainer |
| P-GPU | Existing GPU consumer/integration rows | Compile + current hardware tests with explicitly identified device and fallback behavior | Existing PR/release checks retained; GPU owner; no implementation audit inferred |
| P-BINDINGS | Existing WASM binding/public-surface rows | wasm-pack build, Node tests, discovery surface drift checks | Existing PR/release checks retained; binding/CI owner |

For target compilation:

```sh
cargo +1.98.0 check --locked -p amari-core --lib --target TARGET F
cargo +1.89.0 check --locked -p amari-core --lib --target TARGET F
```

Where a target/toolchain pair is not available, record **BLOCKED**, not pass. A rustup-installed target is required but is not proof of execution. Bare-metal runtime is not promised by library compilation; emulator/board smoke testing and allocator evidence must be separately recorded before claiming an application is install/run-tested there.

Current native-precision documentation explicitly excludes WASM; preserve that restriction. High precision enables Dashu std, native precision needs its platform backend, Rayon depends on std, and the current Serde activation inherits std. Do not call these bare-metal configurations supported, or quietly reject them, without a focused feature-domain decision. Retaining the embedded promise requires a separate dependency/API portability design, not just adding `alloc::Vec` imports. Phase P-EMBED initially establishes the advertised alloc-based minimal/phantom/GF(2) envelope; any expanded precision/serialization/parallel/proof availability on embedded targets requires a precise supported-domain record.

For WASM, probe all native-free canonical closures at the portability gate; formal compatibility is not proof, and Rayon dependency compilation is not proof of usable threads. Threaded execution support must be explicitly established before parallel behavior is advertised for that runner. Tests that require unavailable native capabilities must not be silently treated as successful WASM tests.

Do not execute new unsafe SIMD paths merely because AVX2 is present. Review the allocation/alignment/intrinsics preconditions first; archive the safety review and any applicable Miri/sanitizer evidence separately. Miri cannot be assumed to validate unsupported SIMD/native code. No safety certification follows from green ordinary tests.

**Proposed scheduling, pending concrete matrix approval:** P-AVX2 and P-EMBED are pre-1.0 prerequisites, not conditions claimed repaired by host-only W00. The maintainer selected retaining portability promises and staging actual proofs; the precise portability-row timing proposed here is not independently approved yet. Existing target integration checks still apply to each PR. Do not make 1.0 readiness claims until all these rows are green or the maintainer explicitly revises the support envelope.

## 6. Workspace and downstream integration

Package-level isolated results do not replace repository CI. Retain the exact current workflow contracts:

- `.github/workflows/ci.yml`: stable and pinned-nightly native-precision workspace tests (existing discovery sharding preserved), stable native Clippy/full formatting, high-precision workspace and named-crate tests, WASM checks/build/Node tests, MSRV workspace compilation, rewrite vendored-SMT tests, crate/sharding/catalog/surface verification.
- `.github/workflows/mathematical-correctness.yml`: workspace lib/bin/default/formal-compatibility tests, doctests, integration, discovery shards + aggregator, docs, all-feature Clippy and formatting.
- Scheduled/manual `.github/workflows/parallel-verification.yml`: remains scheduled despite its disabled title; inventory separately, not proof or mandatory PR execution by implication.
- Reporting/badge workflows: source counts are not test or proof passes. Their success is not a mathematical gate.

Direct core consumers discovered via `cargo metadata`: root amari; amari-wasm, automata, dual, tropical, calculus, measure, info-geom, enumerative, functional, fusion, holographic, network, optimization, gpu, probabilistic, relativistic, topology, dynamics, discovery. Record feature unification and transitive consumers, not just search hits. W01 requires resolved affected-caller inspection plus applicable consumer tests; genuine design-changing failures stop the bounded package.

**Remote enforcement observation:** read-only API on 2026-10-09 returned branch-protection 404 for develop/master. The visible active ruleset `8224832` applies deletion/non-fast-forward protection to the default branch; it does not list required status checks. PR #276 nevertheless displayed passing workflow jobs. Do not describe those jobs as server-enforced required checks from this evidence. Repository rules still prohibit direct protected-branch pushes and require reviewed green PRs. Configuring enforcement is a separate maintainer administration task, not permission to bypass CI.

No workflow dispatch, backend omission, blanket lint allow, reduced test assertions, global serialization or forced retry is authorized by this matrix.

## 7. Formal compatibility, static rejection and actual proofs

### C1 — Ordinary annotation compatibility (W00/W01)

Run full H and named formal rows on ordinary stable/nightly. Reachable core modules are `verified.rs` under phantom types and `verified_contracts.rs` under formal. `verified_laws.rs` has no module declaration/inclusion and is dormant. Do not attach it merely to increase counts.

Creusot contracts/proc macros 0.8.0 select dummy implementations outside `cfg(creusot)`. Ordinary `requires`/`ensures` remove contract expressions; a green build does not even establish that those expressions are meaningful for proof translation. Runtime assertions validate only their executed cases.

### C2 — Actual type-boundary rejection (W06 / before 1.0)

Select compile-fail tooling explicitly; no new dependency is implicit. Execute negative dimension/signature/grade/rotor ingress cases only for the guarantees the approved design actually promises. Commented-out examples are inventory, not tests. Check negative controls against the intended public API, not unrelated compiler errors. Record runtime rejection for dynamic boundaries separately.

### C3 — Toolchain conformance (before 1.0)

Recommended initial baseline: upstream **Creusot v0.8.0**, matching contracts 0.8.0 and upstream `nightly-2025-11-13`; official tag-specific installation procedure, not floating `cargo install --git`. Capture release source hash, installation provenance, compiler, prelude, `cargo creusot version`, Why3, why3find, solver versions and configuration. These tools are not installed here, and their exact solver tuple must be recorded when bootstrapped rather than invented now.

Require both a nontrivial supported positive proof fixture and a deliberately false postcondition that remains unproved/fails the gate. The gate must inspect obligation outcomes, not rely solely on process exit success. Version/tool availability alone is insufficient.

### C4 — Core translation and discharged obligations (before 1.0)

The phantom/formal audit and decisions must establish a defensible obligation inventory, trusted assumptions and numeric model first. Exact algebraic identities over real coefficients are not literal associativity of arbitrary IEEE arithmetic. Do not weaken a false contract to an axiom or trust the target function to make a solver pass.

Proposed translation command, to validate during toolchain conformance:

```sh
cargo creusot -- --locked -p amari-core \
  --no-default-features --features std,formal-verification
```

Upstream v0.8 documentation gives `cargo creusot prove` with artifact patterns and a preceding translation step; it does **not** document forwarding ordinary Cargo flags after `prove`. Establish the tested invocation/configuration that proves only the translated approved target. Do not ship an untested `cargo creusot prove -- --locked ...` command as verified tooling.

Require nonempty generated Coma/verification conditions tied to source/lock/tool hashes; all promised obligations discharged; unresolved/timeouts reported; trusted assumptions reviewed; replayable archived proof artifacts. Translation, proving and proof replay are separate evidence. Actual core proof completion remains **BLOCKED** pending tooling and specification audit, not passed or waived.

## 8. Newly established blockers and dispositions

These are bounded matrix-research observations, not completion of the remaining chronological source audit:

| ID | Evidence / defect | Proposed owning work and deadline |
|---|---|---|
| MATRIX-01 | Ordinary formal-feature tests erase annotations; current verified/CI wording implies proof | W09 evidence/claim repair + phantom/formal audit; stop making proof claims immediately; actual proof gate pre-1.0 |
| MATRIX-02 | `verified.rs:568` claims arbitrary dot product nonnegative; valid vectors `[1]`, `[-1]` give `-1` | Phantom/formal specification audit before any core proof claim; not an arithmetic-product defect to bundle into W01 |
| MATRIX-03 | `verified_laws.rs` unreachable; its internal test/function names do not establish the asserted laws | Phantom/formal audit + explicitly approved integration/replacement; before proof closure |
| MATRIX-04 | `geometric_axioms` filter executes zero tests; core verified-contract filter without feature cannot exercise its absent module; badges fall back to source counts | Separate W09/CI evidence-integrity repair before relying on these named suites; no passing/verified labels from source counts |
| MATRIX-05 | True no_std target support blocked by dependency std activation, not just imports; feature-off host tests reveal further missing imports in GF(2)/verified/test modules | W00 bounded host repairs plus separate P-EMBED portability contract/implementation; pre-1.0 |
| MATRIX-06 | Core manifest does not inherit workspace rust-version; local MSRV and target/proof tools not installed | Metadata/MSRV evidence package; MSRV checks at W00/W01, target/proof prerequisites as scheduled |
| MATRIX-07 | Serialized/parallel advertised capabilities lack a corresponding gated core operation surface in this source inventory | API/capability inventory under remaining core audit/W09; no new feature implementation authorized; before claiming capability coverage |
| MATRIX-08 | Visible GitHub configuration does not enforce status checks | Maintainer CI administration; workflow green evidence still required, never substitute absent enforcement for permission |

Do not renumber these into closed CORE findings or claim the original report already audited them. Preserve the historical report and fixture unchanged. Fix suggestions require their own bounded contract and applicable approval.

## 9. Fresh observed results and limits

After resolving missing downloaded artifacts under the unchanged lock:

- Nightly and stable default tests pass: **284 unit/integration + 3 doctests**, none ignored.
- Both compilers pass default-plus-native/full/wasm-alias/serialize+parallel/formal tests and all-feature tests. Runtime counts vary with precision/formal module selection; they do not imply proof.
- Stable default/all-feature warning-denied Clippy passes. Research nightly default/all-feature Clippy fails with the three documented `needless_range_loop` errors.
- Research nightly no-default library check/Clippy fails with missing alloc imports; no-default phantom/GF(2)/formal rows expose additional import/module errors.
- Std-only and optional std singleton all-target checks with GF(2) absent fail on the unconditional integration-test import. `std,gf2` tests pass but warn about precision imports.
- Formatting (`cargo +nightly fmt --all --check`) and default warning-denied rustdoc pass.
- Independent reviewer original-module probe, rerun by the orchestrator, prints `dot=-1` with formal enabled. Published dependency source confirms dummy annotation erasure. No proof tool invoked.
- `cargo test ... geometric_axioms` returns exit 0 with **zero executed tests** in all six emitted targets; this is not a passed axiom suite.

Not executed: full H, every alias equivalence gate, MSRV/pinned-CI-nightly local rows, target builds/runtimes, explicit AVX2 paths, Miri/sanitizers, proof translation/discharge. No full-matrix-green or merge-ready claim is made. Research made no production edits and did not change any workflow or remote setting.

## 10. Proposed owners, milestones and execution handoff

The **operator/core maintainer** is accountable for the matrix/support decisions and all unassigned specialist roles until explicitly delegated. The current session owns W00/W01 contract authoring, generation dispatch, local verification and triage; ordinary CI executes repository integration rows. Portability, unsafe/SIMD and formal-methods work need explicit subplans and operator delegation before execution. Role ownership here is a proposal requiring approval, not an assertion that another person accepted work.

Milestones:

1. Approve this complete matrix and resolve any remaining support/row question.
2. Author a bounded W00 contract against fresh develop. Expand the file list only for actually reproduced host import/test-gating/lint defects; preserve arithmetic/order and forbid API/algorithm/toolchain/workflow changes. New behavioral failures stop that contract.
3. Run RED baseline commands, dispatch generation to named `deepseek`, independently verify the applicable full host/compiler rows, and obtain frontier REVIEW-class scrutiny. Submit a reviewed PR to develop; operator merges.
4. After W00 is integrated, rebaseline and refresh W01's bounded finite-coefficient contract. **Obtain explicit approval of that implementation contract before dispatching W01**; the existing arithmetic plan remains a draft, and matrix approval is not package implementation authorization. Keep W02–W08 decisions separate, follow TDD, run consumer/integration rows and frontier review to convergence (max six rounds). Never self-merge.
5. Complete chronological phantom/formal and unsafe audits; approve separate portability, evidence-integrity and proof-tooling/design contracts. All staged gates complete before 1.0 readiness freeze. A host-gate repair is not closure of advertised portability, safety or proof obligations.

There are **no approved exceptions**. Gate timing is explicit; missing tools/results are BLOCKED or NOT RUN, never green. Any support removal, new dependency, numerical policy, expanded algorithm or release action requires its own decision. D07 approval is neither permission to merge nor to tag/publish.

## 11. Workflow reconciliation and next checkpoint

On 2026-10-09, the orchestrator was forked into `.worktrees/core-matrix`. Read-only checks confirmed the tool cwd, actual parent Pi process cwd, and new session header all point to that worktree; branch `chore/core-verification-matrix`, HEAD `ba1b7719f609e8f0f127d620ba6936cfffb61413`. Reading the writing-plans skill established `currentPhase: plan`, `plan: active` in the worktree's monitor state. TDD remains idle with no tracked source/test files; verification remains false. No guard-state file was manually edited or reset.

The maintainer explicitly lifted the stop for **matrix planning only**: document work/read-only command scrutiny, followed by preparation of a bounded validation/review checkpoint. Planning writes are restricted to this worktree's `docs/plans/`. This is not approval of D07, W00/W01 implementation, executable verification, or another moment dispatch.

Incident record: [ia-toolkit #21](https://github.com/Industrial-Algebra/ia-toolkit/issues/21). The monitor's `plan_tracker init` handler advances to `execute`; do not initialize a task tracker as a planning convenience. Ordinary task-status updates are not workflow-phase transitions. Reloading an already active mapped skill also resets workflow tracking in the installed version; check the actual state and do not use skill reloads to erase history or restrictions.

Next: approve and execute only the [CORE-D07 validation/review checkpoint](2026-10-09-amari-core-matrix-validation-checkpoint.md), explicitly reconcile its verification/review phase(s), validate the corrected recipe, then complete report-only review round 2 against the cumulative draft. Round 1 found 0 P1 / 2 P2 / 0 P3; both documentation corrections are applied but not yet revalidated/reviewed. No convergence or complete-matrix-green claim is made.
