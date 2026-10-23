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

- **S1 — dependency surgery (amari-core/Cargo.toml; R1-revised shape).**
  The drafted member-level `{ workspace = true, default-features = false }`
  is IGNORED by Cargo (default-features belongs to the workspace
  declaration; member-level is a warning — reviewer P1, reproduced).
  Revision: amari-core declares the two deps DIRECTLY (bypassing
  workspace inheritance, matching the existing precedent of
  `rug`/`dashu-float`/`creusot-contracts` in this manifest):
  `num-traits = { version = "0.2", default-features = false }`;
  `thiserror = { version = "2", default-features = false }`; and
  `std = ["num-traits/std", "thiserror/std", …existing]`.
  Blast radius: amari-core only — every OTHER consumer's
  `workspace = true` inheritance and defaults are untouched (the
  workspace-level alternative would have forced explicit std wiring in
  ~7 consumer crates). Version specs resolve to the same lock entries
  (no lock change). Dev-dependencies keep `workspace = true` (dev-deps
  are outside the `--lib` cross graph).
- **S2 — phantom libm wiring (the inherited deferral).**
  `phantom-types = ["num-traits/libm"]`. Inert on host std builds
  (std impls take precedence in num-traits' cfg); provides bare-metal
  `Float` impls. No lock change (libm 0.2.16 already present).
- **S3 — P-EMBED verification matrix (R1-revised: codegen, not
  metadata).** For each of {minimal, `+gf2`, `+phantom-types`,
  `+gf2,phantom-types`} × {stable, pinned nightly, MSRV 1.89.0}:
  **`build --lib --target thumbv7em-none-eabihf`** green (the
  reviewer's P2: `check` emits metadata only — "actual target library
  compile" requires codegen), plus per-combo target dep-tree artifacts
  (the std-free proof is the successful build itself; trees archived).
- **S4 — allocator/link/board documentation + smoke link (R1-revised:
  isolated consumer fixture).** `amari-core/examples/` would drag
  DEV-DEPENDENCIES (criterion's graph) into any cross-build, failing
  before the example source compiles (reviewer P1, reproduced). Revision:
  an ISOLATED consumer fixture crate under the evidence tree
  (`docs/plans/core-matrix-evidence/pembed/smoke/`) with its own manifest
  (`[dependencies] amari-core = { path = "../../../../../amari-core",
  default-features = false, features = ["gf2", "phantom-types"] }`) — no
  dev-deps, no workspace membership. Contents: bump allocator
  (in-tree, ~20 lines), panic handler, constructs a `GF2Matrix` and a
  `VerifiedRotor` (phantom path), links via the target's default
  rust-lld to a real ELF. Documents the requirements: consumers provide
  a global allocator for alloc-dependent APIs; `eabihf` hard-float ABI;
  cortex-m consumers bring their own runtime/board support (amari-core
  is a library, not a board crate).
- **S5 — host F1 re-run (Cargo.toml change invalidates).** Full 320-row ×
  3-quadrant H sweep on retained warm dirs (pinned-runner discipline);
  supplemental singletons+aliases; catalog regenerated WITH an expected
  delta this time (feature-table change); drift gate green; std
  regression rows (default test row, clippy, fmt).
- **S6 — exclusions.** Precision (`rug`/`dashu-float`), `serialize`
  (serde), `formal-verification` (creusot) are NOT claimed target-supported
  and are not verified here; P-WASM rows unchanged; benches/docs target
  checks out of scope; no runtime behavior changes on any host row.

## R2 remediation record (2026-10-23) — scope expansion, honestly earned

Reviewer P1 (reproduced on this worktree with the exact S1/S2 edits,
then reverted): after dep surgery, **29–30 in-tree errors surface across
ALL FOUR rows** — the transcendental float methods (`f64::powf/sin/cos/
tan/sinh/cosh/tanh/exp/ln/log/ln_1p/sqrt/atan/round/fract/powi` and the
f32 mirrors) are **std-only**; `core` floats lack them on this target.
Site census (minimal row): `precision.rs` 9 (the `*_precise` trait-impl
methods), `rotor.rs` 5, `exact_square.rs` 5, `lib.rs` 9 (macro-expanded
float calls); `gf2/clifford.rs` +1 on gf2 rows. My original census never
reached these: dep activation aborted compilation before in-tree
codegen. **Census lesson: measure to codegen depth.**

Reviewer P2 (verified): a fixture crate nested under the workspace
directory without its own `[workspace]` table fails Cargo's workspace
discovery. Fix: the fixture manifest carries an empty `[workspace]`
table (its own root — standard isolated-crate pattern).

### Revised surface

- **S1 (extends): dependency surgery + unconditional `libm = "0.2"`.**
  The direct-declaration shape stands; ADD `libm = "0.2"` as an
  unconditional amari-core dependency with this written justification
  (fleet rule): target-side float dispatch (S2b); same lock version
  0.2.16 already present via num-traits' optional edge (zero new
  resolution); fully inert on host std builds (cfg-gated use).
- **S2 unchanged** (`phantom-types = ["num-traits/libm"]` — num-traits'
  OWN Float impls; separate from our direct libm use).
- **S2b (new): target-compatible math dispatch.** A small internal
  module `math_dispatch` exposing per-function free fns with
  `#[cfg(feature = "std")]` → the inherent methods (bit-identical to
  today's behavior on host — the std arm compiles to exactly the
  current calls) and `#[cfg(not(feature = "std"))]` → `libm::`
  equivalents (`pow→powf/powi`, `ln→log`, `ln_1p`, `fract` via
  `x - trunc(x)` where libm lacks it). All ~29 census sites routed
  through the dispatch; the 12-row S3 build matrix + S5 host sweep are
  the completeness and behavior-preservation gates.
- **S4 (extends): fixture manifest gains an empty `[workspace]` table.**

## R1 remediation record (2026-10-23)

Two P1s, both reproduced before revision: (1) member-level
`default-features = false` under `workspace = true` is ignored by Cargo —
S1 revised to direct declarations in amari-core (zero blast radius
outside amari-core; workspace-level alternative would have touched ~7
consumer crates); (2) `examples/` inherit dev-dependencies into
cross-builds (criterion graph fails before example codegen) — S4 revised
to an isolated consumer-fixture crate with a path dependency and no
dev-deps. One P2: S3 upgraded from `check --lib` (metadata) to
`build --lib` (codegen) — "actual target library compile" means code
generation. Baseline diagnostic counts in the census table are one
parallel-ordering snapshot; the authoritative per-combo failure source
is dep-activation regardless of count.

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
