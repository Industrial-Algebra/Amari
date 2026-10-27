# Pre-0.25.0 dependabot-breach verification — develop HEAD f4f7c6f (2026-10-27)

Question (operator): are the dependabot merges since v0.24.1 breaking for 0.25.0?

## Census — 15 dependabot merges since v0.24.1
- 1 cargo (pollster 1.0, #185) — predates the release graph; **Cargo.lock is
  package-IDENTICAL v0.24.1 → develop HEAD** (added: none, removed: none,
  changed: none). Zero Rust dependency drift.
- 11 npm examples-suite: typescript-eslint 8.71.2, vitejs/plugin-react 6.1.2,
  vite 8.3.4, netlify-cli 27.12.1 (+26.1.0), mantine hooks 9.7.2→9.7.3,
  vitest 5.0.5, globals 17.7.0, mantine/code-highlight 9.4.1, eslint 10.6.0,
  typescript 6.0.3, shiki.
- 2 GitHub Actions: setup-node-7 (#203), cache-6 (#177).

## Fresh local matrix on develop HEAD (rust-matrix.log / npm-matrix.log)
| Gate | Result |
|---|---|
| cargo fmt --check | PASS |
| clippy --workspace --features native-precision --all-targets -D warnings (CI flags) | PASS |
| cargo test (CI flags) | **3,035 passed / 0 failed** |
| amari-core --no-default-features | 437 / 0 |
| amari-discovery catalog gate (`catalog` filter) | 72 / 0 |
| MSRV 1.89.0 check -p amari-core | PASS |
| wasm32-unknown-unknown check -p amari-core | PASS |
| examples-suite npm ci (clean tree) | PASS (1,558 pkgs) |
| examples-suite npm run build | PASS (exit 0) |
| examples-suite vitest 5 | **58 / 58** |
| examples-suite eslint 10 | exit 0 (0 errors, 83 warnings — pre-existing style) |
| examples-suite tsc typecheck | 29 errors — **see bisect below: PRE-EXISTING** |
| CI on bumped actions | green on every run since their merges (#351 head 18ed8c1: 3/3 success) |

## Bisect: examples-suite typecheck
29 errors on develop (TS 5.9.3 effective). The v0.24.1-era tree (git archive →
clean npm ci → npx tsc) produces the SAME 29 with an identical code histogram
(5×TS2304, 1×TS2307, 7×TS2322, 1×TS2345, 8×TS2551, 1×TS2591, 3×TS7005,
3×TS7034) and the same effective TS 5.9.3. Not a dependabot regression.

## Verdict
**Dependabot merges are NOT breaking for 0.25.0.** Rust zero drift + full
matrix green; npm install/build/test/lint green; typecheck failure is
pre-existing debt identical at v0.24.1.

## Pre-existing latents surfaced (not dependabot; candidates for small PRs)
1. amari-discovery `probes::supervisor::tests` — 24 failures anywhere they
   run (since #209, 2026-07-20): `spawn_restricted` does `.env_clear()`
   after `Command::new("python3")`, stripping PATH before exec resolution →
   NotFound. CI never sees them (`--exclude amari-discovery`). Fix: resolve
   the interpreter to an absolute path before env_clear.
2. examples-suite typecheck: config files use process/__dirname/path with
   `types: ["@webgpu/types"]` (no node types by design); plus ~15 src-level
   type errors. Was equally broken at v0.24.1.
3. npm audit: 78 vulns (6 critical) — dev-site dependencies only, not
   shipped artifacts.

Regeneration: rust-matrix.log + npm-matrix.log in this dir; matrix commands
inline in logs' section headers. Bisect recipe: git archive v0.24.1
examples-suite | tar -x; npm ci; npx tsc.

## CORRECTION (2026-10-27, same day) — npm row verified against the wrong tree
The npm verification above ran in the PRIMARY checkout, whose working tree
was NOT develop (older lockfile): "npm ci PASS / build / 58 tests / 29 tsc
errors pre-existing". Verification in the develop WORKTREE showed develop's
examples-suite **cannot npm ci at all** — three peer conflicts (mantine
exact-peer pins across mismatched minors; vitest 5 vs @vitest/coverage-v8
^4; typescript ~7.0.2 vs typescript-eslint <6.1.0), enabled by
@justinelliottcobb/amari-wasm pinned "latest". This IS a dependabot-era
regression (v0.24.1 tree installs clean), invisible to CI (no workflow
installs examples-suite). The v0.24.1-bisect conclusion "29 errors
pre-existing" stands only for the OLD tree; on develop the npm gate was
broken at install level.

Rust row unaffected (Cargo.lock identical v0.24.1→develop; matrix green).

Repairs: PR #354 (discovery supervisor interpreter, 24 tests green)
and PR #355 (examples-suite lattice: pins aligned, pages migrated to
amari-wasm 0.24.1 per-signature API, full suite 24 files / 116 tests,
tsc 0 errors, eslint 0 errors — clean npm ci, deterministic twice).

LESSON: verify against the branch's OWN worktree, never a shared checkout
of unknown ref state. Same family as commit-prose-vs-tree: the claim was
about a tree I never ran against.
