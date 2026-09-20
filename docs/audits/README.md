# Base Rust audit ledger

## Agreed scope (2026-09-19)

- Audit first; fixes are separate, reviewed work. No production behavior changes in this branch.
- Priorities: mathematical correctness, enforceable type/phantom invariants, idiomatic Rust and API contracts, necessary refactors, then measured performance.
- Audit **current implementations**, ordered by the oldest contributions, not historical implementations that have already been replaced.
- **Exclude `amari-gpu` and `amari-wasm`**, per maintainer direction: both face major refactors. See the [0.25–1.0 roadmap](../roadmap/V0_25_0_TO_V1_0_0_RELEASE_SEQUENCE.md) (GPU 0.26; WASM 0.27). Optional GPU paths in otherwise in-scope crates also remain outside this CPU/base-Rust pass. Shared mathematical API defects remain in scope.
- Do not use the Mercury rapid-development workflow for this intensive audit.
- This rolling audit runs concurrently with 0.25+ development. It is not the later release-wide 0.29 freeze/readiness gate.

## Baseline and resumption

First pass: `165d7ccbd6d1aa899babb2d0ae392fa15272e556`, fetched `origin/develop` on 2026-09-19. Workspace version remains **0.24.1** at this commit; 0.25 development is in flight.

Local branch: `chore/core-audit-foundations`. Worktree: `.worktrees/core-audit`.

1. Keep every report attached to an exact commit, toolchain, feature configuration and reproducible input. “Read” is not “correct,” and passing examples do not constitute a proof.
2. Preserve prior reports. On a new pass, fetch and pin a new baseline; use `git diff <old-baseline>..<new-baseline> -- <reviewed-paths>` to identify stale coverage. Include manifest, dependency and shared-kernel changes, not just edits to the reviewed file.
3. Review newly changed dependencies before carrying forward an old result. Never rebase the evidence silently and retain old line references as if they were current.
4. Findings distinguish demonstrated mathematical errors, violated documented contracts, missing input checks, and unmeasured performance opportunities. Fixing a convention mismatch may require a maintainer decision rather than changing an equation blindly.
5. Do not infer TDD history from test presence, formal proof from `verified` naming, or document completeness from a warning-free rustdoc run. Do not mechanically flag normal publishable workspace `path + version` dependencies as ecosystem violations.
6. Mathematical regression checks must compare coefficients or use an independent positive-definite error measure—not the library's indefinite `norm()`/`approx_eq()` being audited.
7. Each fix gets a regression test and separate `fix/*` worktree/PR into `develop`. API/architecture changes require agreement first. No publishing, release-tag work, or protected-branch changes are part of this audit.

## Reports

- [Batch 1: original amari-core foundations](2026-09-19-core-foundations.md)
- [Standalone law reproductions](core-foundations-repro/README.md): expected failures are kept outside the production workspace/test matrix, not ignored or converted into tests that enshrine incorrect behavior.

## Crate chronology

Introduction is the oldest addition of the current crate's `Cargo.toml`, from Git history at the baseline. Same-commit siblings are grouped; use dependency order within a group. Introduction dates are author dates, not release dates. Relocated/pre-crate code needs history tracing when its crate is reached.

| First contribution | Commit | Crate(s) | Status |
|---|---|---|---|
| 2025-09-09 | `1030db4` | **amari-core** | Batch 1 recorded; remainder pending |
| 2025-09-09 | `1030db4` | amari-info-geom | Pending |
| 2025-09-09 | `6ab9287` | amari-dual, amari-tropical, then amari-fusion | Pending |
| 2025-09-18 | `6a1fbae` | amari-automata | Pending |
| 2025-09-29 | `524ba72` | amari-enumerative | Pending; old GF(2) audit is evidence, not a pass |
| 2025-10-05 | `e595757` | amari-relativistic | Pending; roadmap already tracks independent P1 fixes |
| 2025-10-09 | `a796859` | amari-network | Pending |
| 2025-10-21 | `d81e2d6` | amari-optimization | Pending |
| 2025-11-17 | `e539c9b` | amari-flynn-macros, amari-flynn | Pending |
| 2025-11-21 | `d7655eb` | amari-measure | Pending |
| 2025-11-24 | `f0b581b` | amari-calculus | Pending |
| 2025-12-22 | `23482bf` | amari-holographic | Pending |
| 2025-12-22 | `62120e7` | amari-probabilistic | Pending |
| 2025-12-29 | `ba20afc` | amari-functional | Pending |
| 2025-12-31 | `b5832bc` | amari-topology | Pending |
| 2026-01-08 | `00cdfb6` | amari-dynamics | Pending |
| 2026-04-24 | `0292d82` | amari-cgt, amari-surreal | Pending |
| 2026-05-10 | `354a909` | amari-surcomplex | Pending |
| 2026-05-10 | `abd6898` | amari-rewrite | Pending; actively changing |
| 2026-07-10 | `9f057c4` | amari-discovery | Pending; rebaseline before its planned Lonis refactor |
| 2026-07-28 | `caab426` | amari-discovery-macros | Pending |
| 2026-08-12 | `004af19` | amari-rewrite-macros | Pending |
| Initial workspace | — | root amari facade, integration tests, workspace configuration | Context read only; cross-crate integration pass pending |
| 2025-09-09 | `1030db4` | amari-gpu, amari-wasm | **Excluded by maintainer** |

## amari-core file ledger

Paths below are relative to `amari-core/`. All coverage is at the first-pass baseline above. **Read + findings recorded does not mean approved/correct.** Tests may be read ahead of introduction order to validate an older source file. The dedicated phantom/formal modules have NOT yet been audited.

| Introduction | File | Coverage |
|---|---|---|
| 2025-09-09 `1030db4` | `Cargo.toml` | Read; default/minimal build matrix sampled; feature audit partial |
| 2025-09-09 `1030db4` | `src/basis.rs` | Entire file read; batch 1 API/bounds observations |
| 2025-09-09 `1030db4` | `src/cayley.rs` | Entire file read; exhaustive small-signature oracle; performance observation |
| 2025-09-09 `1030db4` | `src/lib.rs` | Entire file read; batch 1 findings and reproductions |
| 2025-09-09 `1030db4` | `src/rotor.rs` | Entire file read; batch 1 findings and reproductions |
| 2025-09-09 `1030db4` | `examples/basic.rs` | Entire file read; CPU 3D example run; illustrative output, not assertions |
| 2025-09-16 `ad5a4e6` | `tests/geometric_product.rs` | Entire file read and default suite run; weak tests noted |
| 2025-09-16 `8643ca9` | `tests/products.rs` | Entire file read and default suite run; assertion-free test noted |
| 2025-09-16 `8643ca9` | `tests/rotors.rs` | Entire file read and default suite run; Euclidean 3D-only coverage |
| 2025-09-16 `deb88f9` | `src/unicode_ops.rs` | **Next source file: pending** |
| 2025-09-30 `0106ae0` | `src/verified.rs` | Pending; Clippy baseline failures recorded only |
| 2025-09-30 `0106ae0` | `src/verified_contracts.rs` | Pending |
| 2025-09-30 `0106ae0` | `src/verified_laws.rs` | Pending; reachability/proof status must be established |
| 2025-09-30 `0106ae0` | `src/property_tests.rs` | Executed under defaults; source audit pending |
| 2025-09-30 `0106ae0` | `src/comprehensive_tests.rs` | Executed under defaults; source audit pending |
| 2025-10-01 `fe317e0` | `src/error.rs` | Pending |
| 2025-10-03 `f05e2e6` | `src/aligned_alloc.rs` | Pending; unsafe/Miri review belongs here |
| 2025-10-03 `f05e2e6` | `src/simd.rs` | Pending; CPU SIMD is in scope |
| 2025-10-03 `f05e2e6` | `benches/performance_suite.rs` | Pending |
| 2025-10-05 `025cb90` | `src/precision.rs` | Pending; minimal-feature warning recorded only |
| 2025-12-21 `ae23d11` | `README.md` | Context searched; full audit pending |
| 2026-02-25 `8e1d854` | `src/gf2/mod.rs` | Pending |
| 2026-02-25 `8e1d854` | `src/gf2/scalar.rs` | Pending |
| 2026-02-25 `8e1d854` | `src/gf2/vector.rs` | Pending |
| 2026-02-25 `8e1d854` | `src/gf2/matrix.rs` | Pending |
| 2026-02-25 `8e1d854` | `src/gf2/grassmannian.rs` | Pending |
| 2026-02-25 `8e1d854` | `src/gf2/clifford.rs` | Pending |
| 2026-02-25 `8e1d854` | `tests/audit_tests.rs` | Entire file read and default suite run; old scope claims rechecked |
| 2026-02-25 `8e1d854` | `tests/gf2_tests.rs` | Default tests executed; ungated import blocks std-only tests; source audit pending |
| 2026-05-23 `c53b2cc` | `src/generic.rs` | Pending; no-default build errors recorded only |

## Immediate next pass

Continue with `unicode_ops.rs`, then the September 30 verified/phantom types and their tests. Check whether public constructors, conversions, arithmetic and deserialization preserve each claimed grade/signature/normalization invariant. Establish which contracts are actually compiled/proved. Do not mark those modules verified merely because the default tests passed.
