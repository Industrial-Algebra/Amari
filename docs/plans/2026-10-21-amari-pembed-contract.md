# P-EMBED Contract — Bare-Metal Target Support (thumbv7em-none-eabihf)

**Branch:** `chore/pembed-contract` from develop `5dc4fc0` · 2026-10-21
**Matrix item:** §5 P-EMBED — "Actual target library compile + target
dependency tree with no target-side std; allocator/link/board requirements
documented. Pre-1.0 portability prerequisite."
**Inherits:** the no-std host repair's deferred libm question (#336/#339).

## Measured surface (census on `5dc4fc0`, cross-checks on this worktree)

**Toolchain targets**: `thumbv7em-none-eabihf` rust-std installed for
stable, pinned nightly, and MSRV 1.89.0 (note: the first `rustup target
add` produced a claimed-installed-but-absent sysroot — remove+add resolved
it; recorded as environment caveat, not a repo defect). The toolchain's
bundled `rust-lld` is the default linker for this target — no system
`arm-none-eabi-gcc` needed.

**Cross-compile failures today** (`check --lib --target thumbv7em-none-eabihf`,
baseline wrapper discipline, `--locked` against the M-H executed lock):

| Combo | Errors | Source |
|---|---|---|
| minimal (no features) | 5 | `thiserror` `extern crate std` ×2 (its default `std` feature), `num-traits` `extern crate std` ×2 (same), +1 summary |
| `+gf2` | 5 | identical — in-tree gf2 code is target-clean post-#339 |
| `+phantom-types` | 5 | identical — in-tree verified.rs is target-clean post-#339 |
| `+gf2,phantom-types` | 5 | identical |

The entire failure surface is **dependency std activation** (MATRIX-05's
original wording, now at target scope): every in-tree no-std defect was
already repaired by #339. `approx` and `bytemuck` compile target-side as
resolved. The phantom-combo target dep graph is exactly:
`approx, bytemuck, num-traits, thiserror` (+creusot absent: phantom alone
does not pull it).

**Upstream facts (vendored sources):** `thiserror 2.0.21` is
`#![no_std]`; `extern crate std` only under its `std` feature (lib.rs:276-279).
`num-traits 0.2.19` activates std impls via its default `std` feature; its
`libm` feature provides the no-std `Float` impls (float.rs:
`cfg(all(not(std), libm))`) — the deferred D1 question, live at target
scope. amari-core uses thiserror's derive unconditionally (core-compatible
output; the `std::error::Error` impls come from its std feature, mirroring
our own cfg-gated impls in error.rs:374-434).

## Surface

- **S1 — dependency surgery (amari-core/Cargo.toml, 4 lines).**
  `num-traits = { workspace = true, default-features = false }`;
  `thiserror = { workspace = true, default-features = false }`;
  extend `std = ["num-traits/std", "thiserror/std", …existing]`.
  Host behavior unchanged in practice (workspace unification provides std
  where present; explicit now where it matters); proven by S5.
- **S2 — phantom libm wiring (the inherited deferral).**
  `phantom-types = ["num-traits/libm"]`. Inert on host std builds
  (std impls take precedence in num-traits' cfg); provides bare-metal
  `Float` impls. No lock change (libm 0.2.16 already present).
- **S3 — P-EMBED verification matrix.** For each of
  {minimal, `+gf2`, `+phantom-types`, `+gf2,phantom-types`} ×
  {stable, pinned nightly, MSRV 1.89.0}:
  `check --lib --target thumbv7em-none-eabihf` green, plus per-combo
  target dep-tree artifacts proving no std-requiring crate (the build
  itself is the proof; trees archived for the record).
- **S4 — allocator/link/board documentation + smoke link.** A no-dep
  smoke binary in `amari-core/examples/` (feature-gated
  `cfg(all(target_arch = "arm", target_os = "none"))`): bump allocator
  (in-tree, ~20 lines), panic handler, constructs a `GF2Matrix` and a
  `VerifiedRotor` (phantom path), links via rust-lld to a real ELF.
  Documents the requirements: consumers provide a global allocator for
  alloc-dependent APIs; `eabihf` hard-float ABI; cortex-m consumers bring
  their own runtime/board support (amari-core is a library, not a board
  crate). The example is `--no-default-features --features gf2,phantom-types`.
- **S5 — host F1 re-run (Cargo.toml change invalidates).** Full 320-row ×
  3-quadrant H sweep on retained warm dirs (pinned-runner discipline);
  supplemental singletons+aliases; catalog regenerated WITH an expected
  delta this time (feature-table change); drift gate green; std
  regression rows (default test row, clippy, fmt).
- **S6 — exclusions.** Precision (`rug`/`dashu-float`), `serialize`
  (serde), `formal-verification` (creusot) are NOT claimed target-supported
  and are not verified here; P-WASM rows unchanged; benches/docs target
  checks out of scope; no runtime behavior changes on any host row.

## Decisions — RESOLVED (operator, ask_user 2026-10-21)

**D1(a): surgery as drafted. D2(a): smoke ELF included.** Both as recommended.

## Decisions (operator) [historical]

- **D1 — dependency surgery shape:** as S1+S2 (recommended), vs
  std-gating thiserror/num-traits usage entirely (much larger diff,
  loses derives/Float on host) vs vendoring (rejected: complexity).
- **D2 — smoke link scope:** include the S4 ELF smoke example
  (recommended; rust-lld is bundled, cost ~one gated example file) vs
  documentation-only (matrix wording permits, but the link proof is cheap
  and closes "allocator/link requirements" empirically).

## Re-verification summary (post-implementation gates)

S3 matrix (24 cross-checks) + S4 ELF build + S5 host sweep (2,880 + 166 +
catalog + std rows) + MSRV target column. Expected post-state: P-EMBED's
§5 evidence row discharged for the four stated combos; H matrix remains
fully green; catalog delta reviewed and committed.
