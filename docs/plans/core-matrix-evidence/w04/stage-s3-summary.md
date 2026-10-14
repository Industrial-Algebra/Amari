# CORE-W04 stage 3 — slerp, `from_vectors`, and reflection (evidence summary)

Worktree `fix/core-w04-rotor-duality`, HEAD `50e9b46e` with the S1 + S2 + S3
changes uncommitted on top. `Cargo.lock` sha256 prefix `eb2442dc`, so the
`--locked` recipe is unchanged. Scope authority:
`docs/plans/2026-10-13-amari-core-w04-rotor-duality-contract.md` §2e, §3, §5,
§7-S3, and the consumer companion
`docs/plans/2026-10-13-amari-core-w04-consumer-impact.md`. Target dir
`target/w04`; recipe pinned (see each `stage-s3-*.log` header):

```
RUSTC_WRAPPER='' RUSTC_WORKSPACE_WRAPPER='' RUSTC=/home/lucien/.nix-profile/bin/rustc \
  RUSTFLAGS='-C target-cpu=x86-64 -C target-feature=-avx2' CARGO_TARGET_DIR=target/w04 --locked
```

S3 delivers the sign-selected slerp, the full `from_vectors` direction
construction (causal type, exact 2×2-minor pre-test, candidate trichotomy), and
the breaking `reflect` (hyperplane, grade involution) / `reflect_line` pair.
Duals and dimension-general components (S4) remain out of scope.

## Files

- `amari-core/src/exact_square.rs` — S3 exact helpers: `ReverseNormState` +
  `exact_reverse_norm_state` (distinct exact `CC† > 0` / `= 0` / `< 0` /
  non-scalar / uncertified outcomes for the §3a trichotomy),
  `exact_reverse_norm_unit` (exact `n·n = ±1` certificate for the §3b normal),
  and `exact_product_eq` (overflow-free exact equality of two dyadic products,
  for the §3a.4 minors).
- `amari-core/src/rotor.rs` — `slerp(&self, o, t) -> Result<Self, RotorError>`
  (§2e: bit-exact endpoint/identity direct returns, coefficient-dot sign rule
  over all coefficients, `A·(A⁻¹B*)^t` interior through `power`, output-`N`
  gate; the hard-coded 8-coefficient loop is deleted);
  `from_vectors`/`from_vectors_mv` full §3a route (certified `σ(x) = x·x`,
  finite-guarded direction normalization, exact proportionality pre-test,
  candidate trichotomy, verified Euclidean-2D antipodal resolution);
  `reflect(n, A) = n·ĉ(A)·n⁻¹` and `reflect_line(n, A) = n·A·n⁻¹` with
  `certified_unit_normal` (`NotAVector` before `NotUnitNormal`) and
  `grade_involution`; migrated `test_reflection` to `reflect_line`, plus three
  new private batteries (candidate identities for both `s` signs; the exact
  coefficient-dot tie).
- `amari-core/tests/rotor_slerp_vectors_reflect_s3.rs` — new public S3 battery
  (21 tests).
- `amari-core/tests/rotors.rs` — `slerp` call sites migrated to the `Result`
  API (`.expect(...)`).
- `docs/plans/core-matrix-evidence/w04/stage-s3-*.log` + this summary.

## Verification (see `stage-s3-*.log`)

| Gate | Result |
|---|---|
| `cargo test -p amari-core --lib --locked` | **234 passed, 0 failed** (S2: 231; +3 S3 unit batteries) |
| `cargo test -p amari-core --locked` | **444 passed, 0 failed** (234 lib + 208 integration + 2 doctests; S2: 420) |
| `cargo check --workspace --all-targets --locked` | pass (only the pre-existing wgpu future-incompat warning) |
| `cargo clippy -p amari-core --all-targets --locked -- -D warnings` | pass, 0 warnings |
| `cargo fmt --all -- --check` | clean, exit 0 |
| `RUSTDOCFLAGS="-D warnings" cargo doc -p amari-core --no-deps --locked` | pass |
| `cargo check -p amari-core --lib --no-default-features --locked` | pass |
| `cargo check -p amari-core --lib --no-default-features --features std --locked` | pass |

The new `rotor_slerp_vectors_reflect_s3` integration binary contributes 21
tests; the lib gains the three private S3 batteries.

## S3 batteries (all green)

Public API (`tests/rotor_slerp_vectors_reflect_s3.rs`, 21 tests):

- slerp: Cl(2,0,0) identity → exact identity for `t ∈ {0, 0.5, 1}`; Cl(4,0,0)
  `identity.slerp(r, 1.0)` bit-exact clone of the single-plane e34 rotor;
  generic endpoint direct returns; the differing-sign fixture
  (`A ≈ 0.6 + 0.8e12`, `B ≈ 0.28 + 0.96e12`, coefficient dot `+0.936`) stays in
  B's hemisphere while the scalar inner product `−0.6` would flip it; the
  negative-coefficient-dot fixture (`A = 1`, `B ≈ −1`) selects `B* = −B` and
  stays near the identity.
- `from_vectors`: Cl(0,2,0) `e1 → e2` action; `e1 → e1` identity (not
  antipodal); `e1 → −e1` resolved in 2D; Euclidean-2D non-axis antipodal
  `e1+e2 → −(e1+e2)` resolved (not the `C ≈ 1.77e−16` fixing-scalar artifact);
  Cl(2,1,0) `−e1+e2+e3` → `Err(NullCandidate)`; Cl(1,1,0)
  `−5/4·e1 + 3/4·e2` → `Err(SheetChange)`; `from_vectors_mv(scalar(1), e1)` →
  `Err(NotAVector)`; null vector → `Err(NullVector)`; mixed causal types →
  `Err(CausalTypeMismatch)`; the `Câ = b̂C` and `C = s·(b̂+â)·â` identities for
  both signs of `s`.
- `reflect`/`reflect_line`: `reflect(e1,e1) = −e1`; `reflect(e1,1) = 1`;
  `reflect(e1,e23) = e23`; `reflect` involution over a blade battery;
  `reflect_line(e1,e1) = +e1`; `NotAVector` before `NotUnitNormal` precedence.

Private stored-field (`src/rotor.rs` unit batteries):

- `direction_candidate` identities `Câ = b̂C` and `C = s·(b̂+â)·â`, asserted
  exactly, for `s = +1` (Cl(2,0,0)) and `s = −1` (Cl(0,2,0)).
- exact coefficient-dot tie: `A = 1`, `B = e12` (Cl(2,0,0)), `⟨A,B⟩_dot = 0`
  keeps `B` (endpoint bit-exact; interior `e12` coefficient positive).

## Deviations / ambiguities resolved

1. **Antipodal resolution condition.** The contract/dispatch parenthetical
   `(P+Q = 2, R = 0)` also admits the mixed 2D signature Cl(1,1,0), but there
   `±e12` are the *identity* action on the plane (both signs), so they cannot
   map `â → −â`. The implementation keeps the `DIM = 2, R = 0` guard and then
   **verifies** the `e12` action against `b̂` before accepting, returning
   `Err(Antipodal)` otherwise. This is the mathematically correct outcome; the
   verified cases are the definite 2D signatures (Cl(2,0,0) / Cl(0,2,0)).
2. **Uncertifiable `σ`/normalization.** Non-finite coefficients in a
   grade-1 input, an unsupported signature (`BASIS_COUNT > 64`), or a
   non-scalar certified square route to `Err(NotNormalizable)` (the contract
   names only the finite fixtures). The `NotAVector → NullVector →
   CausalTypeMismatch → antipodal/trichotomy` order is unchanged.
3. **`CC†` non-scalar / uncertified.** A candidate whose exact `CC†` is
   non-scalar or leaves the dyadic window routes to `Err(NotNormalizable)`
   (the three named outcomes — positive, zero, negative — are exact and
   certified).
4. **slerp interior normalization.** The interior result `A·(A⁻¹B*)^t` is run
   through `normalize_and_certify`, which enforces the required output-`N` gate
   (`Err(Degenerate)` on collapse) and returns a stored-unit rotor. The
   endpoints and the `A = B` case remain bit-exact direct returns, so the
   enumerated bit-exact pins are unaffected.
5. **Proportional same-direction inputs.** These fall through to the
   candidate trichotomy (contract §3a.4), which yields the identity rotor for
   `C = 2`-type cases; no separate short-circuit is added.
6. **`reflect` normal certificate.** `n·n = ±1` is certified on the exact
   dyadic magnitude (a rounded `sqrt` check could accept a value within half an
   ulp of 1), so `n⁻¹ = n/(n·n)` is exact.
7. **Existing `test_reflection` migration.** The baseline test asserted the old
   `reflect(v, n) = n v n` line map; it now calls `reflect_line(n, A)` and keeps
   its `e1`/`e2` assertions, and additionally pins `reflect_line(e1, e1) = +e1`
   verbatim.
8. **Tie fixture placement.** The exact `⟨A,B⟩_dot = 0` tie cannot be built
   through the public constructors (no arbitrary even-rotor constructor);
   it is a private-stored-field unit battery. The public differing-sign and
   negative-dot fixtures use `from_bivector` with `angle = −2·asin(b)`, whose
   rounded coefficients keep the dot signs the fixtures pin.
