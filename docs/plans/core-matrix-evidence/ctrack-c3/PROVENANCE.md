# C3 Toolchain Provenance (recorded as OBSERVED at bootstrap, 2026-10-26)

## Source (tag-specific, official)
- Creusot repository tag **v0.8.0**, rev `77f3e3e204b4d0ceecbfe5b91df8f2a3fe71165f`
  (clone --depth 1 --branch v0.8.0; `./INSTALL` from that tree)
- rust-toolchain at tag: **nightly-2025-11-13** (matches matrix §7 C3)
- rustc: nightly-2025-11-13 via rustup (`rustc +nightly-2025-11-13 -vV` in gate logs)

## Installed components (versions as printed by the tools)
- cargo-creusot: 0.8.0 (cargo install --path cargo-creusot @ v0.8.0)
- creusot-rustc: at `$XDG_DATA_HOME/creusot/toolchains/nightly-2025-11-13/bin/creusot-rustc`
- Why3: **1.8.2+git** (pin: git+https://gitlab.inria.fr/why3/why3.git#51e0579b per creusot-deps.opam)
- why3find: **v1.2.0+dev** (pin: git+https://git.frama-c.com/pub/why3find.git#833defb4)
- OCaml: 5.3.0 (opam switch, per creusot-deps.opam pin)
- Provers (downloaded prebuilt by INSTALL): z3 4.15.3, cvc5 1.3.1, CVC4 1.8, alt-ergo 2.6.2
- Why3 conf: 14 provers registered; **cvc5 1.3.1 flagged "unrecognized version"**
  (known versions ^1.[0-2].x) — caveat recorded; the conformance gate passed
  with the registered set anyway.
- Prelude: creusot prelude via `prelude-generator` @ v0.8.0 →
  `$XDG_DATA_HOME/creusot/share/why3find/packages`; why3find.json +
  creusot_why3.conf installed by INSTALL.

## Environment accommodations (NixOS; deviations disclosed)
1. `opam option depext=false` + `--assume-depexts`: opam cannot drive
   the NixOS system package manager. Build deps supplied by nix:
   gcc/cc, gmp.dev, zlib.dev, autoconf, patchelf (nix profile).
2. Build env: `CPATH/LIBRARY_PATH/PKG_CONFIG_PATH` → ~/.nix-profile
   (opam conf-packages probe via cc/pkg-config).
3. **why3-ide SKIPPED** (GUI; pulls gtk3/cairo stack — conf packages
   cannot pass on NixOS without the GUI libs). CLI tooling complete:
   why3 + why3find + provers + prelude. why3-ide is not used by
   cargo-creusot prove.
4. Downloaded prover binaries are glibc-linked: **patchelf
   --set-interpreter** (nix glibc 2.42-67) + rpath → they run and
   register.

## Conformance gate (S6) — BOTH directions, outcome-parsed
- Positive: `sum_to` (u64 loop with invariant) — `Library
  verif.creusot_conf_rlib.sum_to: ✔ (1)` — proved.
- Negative: `wrong_successor` (`#[ensures(result@ == n@)]` on `n+1`) —
  `Goal Coma.vc_wrong_successor: ✘`, `Error: 1 unproved file` — the
  false postcondition stays UNPROVED and the tool fails it.
- Gate semantics: parse ✔/✘ per goal; a green exit with ✘ present, or
  a red exit with only ✔, is a gate failure.

## S8 translation smoke — honest split
- **Toolchain-level**: the conformance fixture translates NONEMPTY and
  **bit-identically across a clean replay** (fresh target+lock): 1
  coma artifact, set sha256 `8d03a32b…` both runs.
- **amari-core surface (via carrier fixture, isolated workspace)**:
  translation **FAILS deterministically** — rc 101, 7 logic-mode type
  errors both runs, identical error sets. Families: plain-Rust methods
  called in specs (`is_scalar`, `is_even_grade`, `grade`, `dimension`
  — must be `#[logic]` fns), `T - T` (generic arithmetic has no logic
  impl), Result-in-spec mismatches. The matrix C1 warning is thereby
  DEMONSTRATED on the real surface: 23 annotations are dummy-path-only.
- Consequence recorded for C4-full: annotation repair (logic functions,
  snapshots, numeric model) is a prerequisite — the §7 BLOCKED status
  now has a concrete, evidenced reason.
