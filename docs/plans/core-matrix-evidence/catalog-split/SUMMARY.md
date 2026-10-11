# CORE-CAT1 evidence — per-crate catalog split

Branch `chore/catalog-split` from develop `1d2bfa9` (post-#285, post-#286).
Branch lock `a3fdc0e0…` (Cargo.lock is gitignored; per-worktree resolution).

## What changed

- `amari-discovery/src/catalog/split.rs` (new): `CatalogManifest`,
  `SplitTotals`, `CrateFileEntry`, `split_catalog`,
  `compose_structural`. Split/compose are exact inverses.
- `examples/generate_catalog.rs`: writes `catalog/manifest.json` +
  `catalog/crates/<name>.json` (28 files) + generated `catalog/index.rs`
  (documented include table); removes the legacy monolith and stale
  per-crate files; atomic tmp+rename writes preserved.
- `src/catalog/mod.rs`: `embedded()` composes the split layout and
  serializes canonically before the unchanged `from_sources` path —
  `content_hash` recipe and validation semantics unchanged.
- `verify_checked_in(root, catalog_dir)`: split-aware; drift message
  names the drifted file(s); orphaned crate files and a lingering
  monolith count as drift.
- Tests: new `tests/catalog_split.rs` (3 contracts); `catalog_integrity`
  and `catalog_generation` adapted; CI drift check widened to the whole
  `amari-discovery/catalog/` directory.

## Proofs

1. **Round-trip byte equality** (TDD red-first: E0432 before the API
   existed): `split_catalog` → `compose_structural` reproduces the
   canonical monolith serialization byte-for-byte → `content_hash`
   unchanged in meaning. Split tests 3/3.
2. **Determinism**: repeated generation leaves the catalog tree
   byte-identical (hash `3b5c2aec…`, 28 crates / 11,096 items / 107
   edges — same counts as the pre-split monolith `f2b25c34` layout).
3. **Collision isolation** (the property this PR exists for): a scratch
   `pub fn` added only to `amari-rewrite` changed exactly
   `crates/amari-rewrite.json` + `manifest.json`; the other 27 crate
   files and `index.rs` were untouched. Probe reverted; tree verified
   byte-identical to the pre-probe snapshot.
4. **Environment-local failure identity**: `-p amari-discovery --lib`
   shows 191 passed / 24 failed, all `probes::supervisor`; the same 24
   fail identically on clean develop `1d2bfa9` (temp worktree
   /tmp/develop-baseline) — pre-existing environment failures, CI-green.

## Gates

Catalog suites: split 3/3, generation 41/41, integrity 11/11. (Historical counts transposed in the original record — round-2 review P3, corrected here.) Workspace
all-targets check 0. fmt clean. Clippy `-D warnings` 0 on
lib/bins/examples + the three catalog test targets; the only local-1.98
clippy failures are the three pre-existing dead-code lints in
`tests/probe_rewrite_inverse_search.rs` (file untouched by this branch —
`git diff` empty; same disposition as W02's record). Lock unchanged
after generation.

## Bootstrap note

The loader includes generated `catalog/index.rs` at compile time; a
fresh workspace (no index.rs yet) bootstraps by stubbing the include
once, running the generator, then restoring the include. Bootstrap
machinery is not committed — the generator output is.

## Review round 1 — findings and remediation

Round 1 returned **1 P1 / 3 P2** — all verified real, all remediated:

1. **P1 `rewrite_discovery_macros.rs` still read the removed monolith** (my consumer search missed it). Fixed: the test composes the embedded split sources (`compose_structural` over the generated include table) — 5/5 green. Guide and benchmarks documentation updated to the split layout; remaining `generated.json` references are historical plans/release notes, intentionally untouched.
2. **P2 verifier ignored `index.rs`.** Fixed: `render_index_rs` (and `manifest_bytes`) extracted into the lib as shared machinery; the generator example and `verify_checked_in` both use them, so index corruption/absence is drift. New test `corrupted_index_rs_is_drift` pins it.
3. **P2 composition accepted duplicate manifest refs and silently dropped the replaced crate.** Fixed: duplicate references rejected; every supplied body must be consumed exactly once (unreferenced bodies rejected by name). Two new split tests pin both. Process disclosure: the first application of this fix was silently lost to a stale-variable overwrite in my edit script (two "success" prints, one write) — caught by the new tests failing, re-applied with a persistence assertion. The interim test failure was my own harness, not the reviewer's finding regressing.
4. **P2 corruption fixtures were already drifted (non-canonical manifest bytes) and failure modes were uncovered.** Fixed: `write_split_with_corruption` writes canonical manifest/index via the shared machinery; new `clean_split_fixture_passes_verification` establishes the baseline; added missing-file, orphan-file, lingering-monolith, and index-corruption tests (generation suite 41→46).

Post-fix gates: generation 46/46, integrity 11/11, split 5/5, macros 5/5; scoped clippy 0; workspace all-targets check 0; fmt clean; catalog tree **byte-identical** through the remediation (hash `3b5c2aec…` unchanged; the fixes are machinery-only).

## Review round 2 — convergence

Round 2 returned **0 P1 / 0 P2 / 1 P3** with verdict "Ready for PR." The P3 (the pre-round-1 gate counts above transposed integrity/generation) is corrected in place. Loop: R1 1/3/0 → R2 0/0/1, converged at 0 P1/P2. (An earlier "1/1/0" shorthand undercounted round 1 — corrected after the PR review flagged it.) Reviewer independently verified: all 33 catalog files match the archived pre-change checksums; fresh suites 46/11/5/5; shared renderer genuinely single-source; all failure-mode regressions pin.

## PR review (posted on #287) — 1 P1 / 0 P2 / 2 P3, remediated

1. **P1 `catalog_split` missing from CI shard assignments** (reproduced in failed CI by the reviewer). Fixed: added to the `catalog` shard in `scripts/run-discovery-test-shard.py`; `scripts/verify-discovery-ci-sharding.py` (the same script CI's "Verify discovery CI sharding" step runs) now verifies 70 targets across 3 shards, exhaustive and stable.
2. **P3 review-loop shorthand** — SUMMARY round-2 section said "R1 1/1/0"; round 1 was 1 P1 / 3 P2 / 0 P3. Corrected with an in-place note.
3. **P3 stale CI diagnostic** — drift error message still named the deleted `catalog/generated.json`. Updated to name the split layout.
