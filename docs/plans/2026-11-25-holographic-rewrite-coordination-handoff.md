# Handoff: holographic audit → rewrite session

**From:** the holographic 1.0 audit programme (operator coordination channel)
**To:** amari-rewrite session
**Status:** H2 implementation is STOPPED pending one migration in your module.
**Correction of record:** the holographic audit DOES impact rewrite — three
landed changes already ran against your gates, and one in-flight change is
a hard compile break in `recall.rs`.

## 1. What has already landed on develop (verified neutral for you)

| Change | PR | Impact on rewrite |
|---|---|---|
| H1: CL(3) kernel fix (16/64 basis-product signs, checked inverse, named duals) | #387 | Your module is trait-level (`BindingAlgebra`/`FHRRAlgebra`, `encode.rs:77`) — no direct `Cl3`/`ProductClifford` use. FHRR convolution was already correct. Your `holographic-guidance` suite passed on the H1 tree (527 at the time). **Caveat:** any stored bindings/traces built through Product-Clifford backends before H1 are stale — 16 blade-pair signs changed; rebuild from operands (procedure: `docs/plans/holo-h1-impl-notes.md`). |
| H3: additive memory, evidence-typed membership, sided recovery | #389 | Memory semantics changed (attention → additive trace; `confidence` is GONE from discovery probes — replaced by `membership_evidence` + nullable `nearest_key_similarity`). Your gates passed (aggregate now 560 with task-48 tests). **Check:** any code of yours reading a discovery probe `confidence` field must move to the v2 fields; the probe id is now `amari-probe:holographic:recall:v2`. |
| Catalog interleaves | — | Task-48's surface entries rode our regen merges; no action. |

## 2. What is in flight and REQUIRES your action: H2

The converged contract is on develop: `docs/plans/2026-11-25-amari-holographic-h2-binding-domain-contract.md` — its **Frozen signatures table and D1–D6 decisions are the authoritative surface-change list**. The H2 implementation branch (`feature/holo-h2-impl`) is stopped before its first unit because of:

### 2a. Required migration in your module (the blocker)

`amari-rewrite/src/holographic/recall.rs:668` — `lane_similarity` calls
`BindingAlgebra::similarity` and immediately clamps the returned `f64`. H2's
D3 makes the trait signature fallible:

```rust
fn similarity(&self, other: &Self) -> AlgebraResult<f64>;
```

This is a compile-time break (E0599 at `.clamp(-1.0, 1.0)` — confirmed by
isolated compilation under 1.100.0). **Per the operator (2026-11-25): the
rewrite session migrates this call site.** The agreed zero-norm-lane policy:
`similarity` returns a typed error for a degenerate lane; your ranking loop
decides skip-vs-fail. On Euclidean lanes the migration is behavior-neutral
(`Ok` everywhere); your goldens should be unchanged.

Suggested migration shape (yours to own):

```rust
let raw = self.similarity(&lane)?;
// or, if a degenerate lane should be skipped rather than fail the ranking:
let raw = match self.similarity(&lane) { Ok(v) => v, Err(e) if e.is_degenerate_lane() => continue, Err(e) => return Err(e) };
```

Coordinate the exact error-type surface with H2's U2 error vocabulary
(`EmptyDimension`, `NonFinite`, `UnsupportedSignature` — see the contract's
API table).

### 2b. Things in your module to audit against the H2 surface (no action unless you use them)

- **`permute` (positional)** — D6 renames the operation to bin-reindex
  semantics (`rotate_bins`/`permute_bins`, checked bijective maps). Your
  positional consumption migrates at the name level with **zero behavior
  change**; if H2's implementation would change behavior, H2 stops and
  coordinates rather than proceeding.
- **`logsumexp`** — the historical `β≤0 → mean` branch is REMOVED
  (it was a fake continuity: the true β→0 limit diverges). If you call it
  with `β=0` expecting the mean, switch to `log_mean_exp` (whose β→0 limit
  is the arithmetic mean and which defines β=0 as the mean).
- **`similarity` on non-Euclidean signatures** — returns
  `UnsupportedSignature`; only Euclidean backends compute a cosine.
- **`bundle` pair-fold assumptions** — bundling is now a whole-list
  operation; the pair form is exactly `bundle_all([a,b])`. No associativity
  or partition-invariance is claimed (a weighted mean is provably
  non-associative). If any of your code assumes fold-composition of
  bundling, it was already relying on undefined behavior.
- **Typed keys / `unbind_general` / `correlation`** — new surface; adoption
  optional. If your ingestion paths bind with arbitrary elements and later
  unbind, the checked-key classes are the supported route.

## 3. Standing protections (unchanged)

1. Every holographic PR runs `cargo test -p amari-rewrite --features
   holographic-guidance` (aggregate 560 at task-48's tree) — your suite is
   a required gate in our loop, not an afterthought.
2. Holographic PRs make NO rewrite edits except the agreed `recall.rs`
   migration; any rewrite-visible behavior or golden change is a
   stop-and-coordinate event, escalated to the operator.
3. After your `recall.rs` migration lands, rerun your goldens and the
   aggregate on the H2 branch head before H2 resumes — the standing gate
   will do it again when H2 opens its PR.

## 4. Sequencing

1. You migrate `recall.rs:668` (+ zero-norm policy per above) on your next
   rewrite PR; it lands on develop.
2. H2 implementation resumes on the merged tree (we rebase
   `feature/holo-h2-impl`), runs the full loop, opens its PR.
3. H4 (optical/Lee) and H5 (selectors/portability, B1 no_std) contracts
   follow; same protections. B1 may touch holographic's feature surface —
   your `holographic-guidance` feature is std-based, so no expected impact.

## 5. Where the truth lives

- Audit register + replayable evidence: `docs/audits/2026-11-18-amari-holographic-audit.md`, `docs/audits/holographic-repro/`
- H1 contract/impl: `docs/plans/2026-11-24-amari-holographic-h1-kernel-contract.md`, `docs/plans/holo-h1-impl-notes.md`
- H3 contract/impl: `docs/plans/2026-11-24-amari-holographic-h3-memory-contract.md`, `docs/plans/holo-h3-impl-notes.md`
- H2 contract (the surface-change law): `docs/plans/2026-11-25-amari-holographic-h2-binding-domain-contract.md`

Questions and stop-events route through the operator, as always.
