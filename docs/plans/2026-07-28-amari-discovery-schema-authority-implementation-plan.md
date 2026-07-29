# amari-discovery 0.24.1 Schema Authority Implementation Plan

> **REQUIRED SUB-SKILL:** Use the executing-plans skill to implement this plan task-by-task.

**Goal:** Give all executable `amari-discovery` probes machine-readable, hash-identified wire schema contracts while preserving the shipped `amari.discovery/v1` protocol.

**Architecture:** Implement Amari-local, trait-first DTO contracts behind a bounded `WireContract` derive. Collect DTO contracts into a deterministic schema registry, canonicalize and hash hybrid structural/semantic documents, expose compact hashes through probe description, and resolve full documents through an additive probe schema command.

**Tech Stack:** Rust 2021, `serde`, `serde_json`, `schemars`, `syn`/`quote`/`proc-macro2`, `trybuild`, `sha2`, existing `amari-discovery` catalog/probe/CLI machinery.

---

## Execution rules

1. Work in a dedicated worktree from updated `origin/develop`.
2. Use TDD: write each failing test, observe the expected failure, implement the minimum code, observe it pass.
3. Preserve `amari.discovery/v1`, existing probe IDs, existing schema IDs, saved plans, output modes, and all probe execution behavior.
4. Do not depend on or copy the current Lonis implementation.
5. Do not add general-purpose schema support beyond the DTO shapes used by current executable probes.
6. Do not run the legacy aggregate `run_all_tests.sh` and do not reintroduce `amari-gpu` runtime tests.
7. Approved verification commands are the release matrix plus targeted tests:
   - `cargo test -p amari-discovery --all-features`
   - `cargo test --workspace --exclude amari-gpu`
   - `cargo test -p amari-discovery-macros --all-features`
   - warning-denied normal targets; all-target Clippy may use the documented tagged-test exclusions.
8. Regenerate the structural catalog only when workspace package/API shape changes. Verify determinism after regeneration.
9. Frequent commits are required; no direct push to `develop` or `master`.
10. Design PRs and implementation PRs do not authorize a 0.24.1 release. Gates A and B remain mandatory later.

## Task 1: Establish the 0.24.1 implementation worktree

**Files:**
- Worktree: `.worktrees/amari-schema-authority-impl-0.24.1`
- Branch: `feature/amari-schema-authority-0.24.1`

**Step 1: Verify post-design baseline**

Run:
```bash
cd /home/elliotthall/working/industrial-algebra/amari
git fetch origin develop --prune
git log -1 --oneline origin/develop
git merge-base --is-ancestor 1565f70 origin/develop
```
Expected: latest `origin/develop` contains PR #228 merge `1565f70`.

**Step 2: Create worktree**

Run:
```bash
git worktree add -b feature/amari-schema-authority-0.24.1 \
  .worktrees/amari-schema-authority-impl-0.24.1 origin/develop
cd .worktrees/amari-schema-authority-0.24.1
git status --short --branch
```
Expected: clean branch tracking `origin/develop`.

**Step 3: Confirm baseline discovery state**

Run:
```bash
cargo run -q -p amari-discovery --bin amari -- capabilities --json \
  | jq '{known:(.data.known_probes|length), executable:([.data.known_probes[]|select(.executable)]|length)}'
```
Expected: `{ "known": 14, "executable": 13 }`.

## Task 2: RED-test wire-contract public model

**Files:**
- Create: `amari-discovery/tests/wire_contract.rs`
- Modify: `amari-discovery/src/lib.rs`
- Create later: `amari-discovery/src/wire/mod.rs`

**Step 1: Write failing model tests**

Create tests requiring:

```rust
#[test]
fn schema_role_serializes_as_input_or_output() { /* input/output */ }

#[test]
fn compatibility_class_serializes_as_stable_snake_case() { /* additive_patch */ }

#[test]
fn schema_summary_rejects_malformed_hash() { /* 64 lowercase hex required */ }

#[test]
fn canonical_document_json_is_pretty_with_trailing_newline() { /* deterministic */ }
```

**Step 2: Run RED**

Run:
```bash
cargo test -p amari-discovery --test wire_contract --all-features
```
Expected: compile failure because wire-contract types do not exist.

**Step 3: Implement minimal model**

Create `src/wire/mod.rs` with extraction-ready types:

- `WireSchemaRole::{Input, Output}`
- `WireCompatibility::{AdditivePatch, VersionedChange}`
- `WireSemanticConstraint`
- `WireExample`
- `ProbeSchemaSummary`
- `ProbeSchemaDocument`
- canonical JSON + hash helpers.

Export from `src/lib.rs`.

**Step 4: Run GREEN**

Run:
```bash
cargo test -p amari-discovery --test wire_contract --all-features
```
Expected: pass.

**Step 5: Commit**

```bash
git add amari-discovery/src/wire/mod.rs amari-discovery/src/lib.rs \
  amari-discovery/tests/wire_contract.rs
git commit -m "feat: add probe wire schema model"
```

## Task 3: Scaffold `amari-discovery-macros`

**Files:**
- Modify: `Cargo.toml` workspace members and dependencies
- Create: `amari-discovery-macros/Cargo.toml`
- Create: `amari-discovery-macros/src/lib.rs`
- Create: `amari-discovery-macros/tests/ui.rs`
- Create: `amari-discovery-macros/tests/ui/pass_simple.rs`
- Create: `amari-discovery-macros/tests/ui/fail_missing_id.rs`

**Step 1: RED workspace/package test**

Add a test asserting cargo metadata includes `amari-discovery-macros` before `amari-discovery` in publish-order terms.

Run:
```bash
python3 scripts/verify-publish-order.py
```
Expected: failure because the crate does not exist.

**Step 2: Create minimal proc-macro crate**

Add:

```toml
[lib]
proc-macro = true
```

Add workspace dependency:

```toml
amari-discovery-macros = { path = "amari-discovery-macros", version = "0.24.0" }
```

Add `.github/workflows/publish.yml` entry before `amari-discovery`.

**Step 3: Run publish-order GREEN**

Run:
```bash
cargo metadata --no-deps --format-version 1 >/dev/null
python3 scripts/verify-publish-order.py
```
Expected: `publish order is dependency-safe`.

**Step 4: Commit**

```bash
git add Cargo.toml amari-discovery-macros .github/workflows/publish.yml
git commit -m "chore: scaffold discovery wire contract macros"
```

## Task 4: RED-test bounded `WireContract` derive

**Files:**
- Modify: `amari-discovery-macros/src/lib.rs`
- Modify: `amari-discovery-macros/tests/ui.rs`
- Create: pass/fail `trybuild` fixtures

**Step 1: Write compile tests**

Pass cases:

- plain named-field struct;
- nested named-field struct;
- internally tagged enum used by rewrite terms;
- `serde(rename_all = "snake_case")` enum;
- fixed arrays, `Vec`, `Option`, `String`, integers, `f64`, booleans.

Fail cases:

- missing `#[wire_contract(id = "...", role = "input|output")]`;
- invalid role;
- tuple struct;
- unit struct;
- unsupported Serde container attributes;
- generic DTO without explicit supported concrete use;
- malformed schema ID.

**Step 2: Run RED**

Run:
```bash
cargo test -p amari-discovery-macros --all-features
```
Expected: missing derive macro / compile-fail fixtures fail.

**Step 3: Implement bounded derive**

Implement `#[proc_macro_derive(WireContract, attributes(wire_contract))]`:

- parse only supported `wire_contract` arguments;
- generate `schemars::JsonSchema`-backed structural schema expression;
- emit trait metadata for ID, role, semantic constraints, examples, compatibility;
- emit deterministic compile-time errors through `syn::Error::to_compile_error`;
- no filesystem, network, environment, time, or random access.

**Step 4: Run GREEN**

Run:
```bash
cargo test -p amari-discovery-macros --all-features
```
Expected: pass/fail fixtures behave as expected.

**Step 5: Commit**

```bash
git add amari-discovery-macros
git commit -m "feat: derive bounded wire contracts"
```

## Task 5: Implement schema document canonicalization and hashes

**Files:**
- Modify: `amari-discovery/src/wire/mod.rs`
- Create: `amari-discovery/tests/wire_schema_document.rs`

**Step 1: RED document tests**

Require:

- document contains `$id`, JSON Schema draft marker, role, protocol marker,
  constraints, examples, compatibility;
- `additionalProperties` is false for `deny_unknown_fields` structs;
- canonical bytes are stable across construction order;
- hash is lowercase SHA-256 over canonical bytes;
- malformed schema ID/role/version is rejected.

**Step 2: Run RED**

```bash
cargo test -p amari-discovery --test wire_schema_document --all-features
```
Expected: failures for unimplemented validation/hash behavior.

**Step 3: Implement document builder**

Use `serde_json::to_vec_pretty` plus trailing newline and `sha2::Sha256`. Add metadata without changing `SCHEMA_V1`.

**Step 4: Run GREEN**

Same command; expected pass.

**Step 5: Commit**

```bash
git add amari-discovery/src/wire/mod.rs \
  amari-discovery/tests/wire_schema_document.rs
git commit -m "feat: canonicalize wire schema documents"
```

## Task 6: RED-test probe schema registry

**Files:**
- Create: `amari-discovery/tests/wire_schema_registry.rs`
- Create: `amari-discovery/src/wire/registry.rs`
- Modify: `amari-discovery/src/wire/mod.rs`, `src/lib.rs`

**Step 1: Write registry tests**

Require:

- every executable probe has one input and one output schema summary;
- every known non-executable descriptor reports contract `declared` rather than
  pretending a compiled DTO exists;
- duplicate schema ID is catalog corruption;
- role/version mismatch is catalog corruption;
- descriptor/adapter disagreement is catalog corruption;
- summary includes ID, role, compatibility, hash, and resolution availability.

**Step 2: Run RED**

```bash
cargo test -p amari-discovery --test wire_schema_registry --all-features
```
Expected: missing registry APIs.

**Step 3: Implement registry**

Add `ProbeWireSchemaRegistry`, built alongside `ProbeRegistry` from catalog descriptors and compiled adapter contracts.

**Step 4: Run GREEN**

Same command; expected pass.

**Step 5: Commit**

```bash
git add amari-discovery/src/wire amari-discovery/src/lib.rs \
  amari-discovery/tests/wire_schema_registry.rs
git commit -m "feat: register probe wire schemas"
```

## Task 7: Convert simple numeric and integer probes

**Files:**
- Modify: `amari-discovery/src/probes/{cgt,core,dual}.rs`
- Modify: `amari-discovery/tests/probe_{cgt,core,dual}.rs` or focused schema tests
- Modify: `amari-discovery/Cargo.toml`

**Step 1: RED tests for three probes**

Require schema resolution and stable hashes for:

- CGT nim sum input/output;
- Cl(3,0,0) geometric product input/output;
- dual polynomial derivative input/output.

**Step 2: Run RED**

```bash
cargo test -p amari-discovery --test wire_schema_registry --all-features \
  cgt_core_dual_contracts
```
Expected: missing DTO contracts.

**Step 3: Derive contracts on DTOs**

Add `schemars::JsonSchema` and `WireContract` derives/attributes. Add semantic constraints:

- finite numbers;
- nonnegative/unsigned bounds;
- coefficient length;
- heap limits;
- nonempty coefficients.

Keep runtime validation unchanged.

**Step 4: Run GREEN**

Run focused tests, then:

```bash
cargo test -p amari-discovery --all-features
```
Expected: pass.

**Step 5: Commit**

```bash
git add amari-discovery/Cargo.toml amari-discovery/src/probes/{cgt,core,dual}.rs \
  amari-discovery/tests
git commit -m "feat: add wire contracts to numeric probes"
```

## Task 8: Convert structured graph/optimization/tropical probes

**Files:**
- Modify: `amari-discovery/src/probes/{network,optimization,tropical}.rs`
- Modify/create focused schema tests

**Step 1: RED**

Require rectangularity, finite weights, source/target bounds, direction cardinality, nonempty observations, and output shape metadata in exported semantic constraints.

**Step 2: Run RED**

```bash
cargo test -p amari-discovery --test wire_schema_registry --all-features \
  structured_contracts
```
Expected: missing contracts.

**Step 3: Derive/annotate DTOs**

Support nested `Vec`, `Option<f64>`, enums, nested output structs.

**Step 4: Run GREEN**

```bash
cargo test -p amari-discovery --all-features
```
Expected: pass.

**Step 5: Commit**

```bash
git add amari-discovery/src/probes/{network,optimization,tropical}.rs \
  amari-discovery/tests
git commit -m "feat: add wire contracts to structured probes"
```

## Task 9: Convert exact rational and holographic probes

**Files:**
- Modify: `amari-discovery/src/probes/{surreal,holographic}.rs`
- Modify/create focused schema tests

**Step 1: RED**

Require decimal string bounds, nonzero denominator/divisor semantics, MAP dimension, entry limits, seed integer types, finite output metrics.

**Step 2: Run RED**

```bash
cargo test -p amari-discovery --test wire_schema_registry --all-features \
  rational_holographic_contracts
```
Expected: missing contracts.

**Step 3: Derive/annotate DTOs**

Handle nested decimal rational/surcomplex structs, holographic entries, attribution, capacity, warnings.

**Step 4: Run GREEN**

```bash
cargo test -p amari-discovery --all-features
```
Expected: pass.

**Step 5: Commit**

```bash
git add amari-discovery/src/probes/{surreal,holographic}.rs amari-discovery/tests
git commit -m "feat: add wire contracts to rational holographic probes"
```

## Task 10: Convert rewrite probes

**Files:**
- Modify: `amari-discovery/src/probes/rewrite.rs`
- Modify/create rewrite schema tests

**Step 1: RED**

Require internally tagged recursive term schema, checked-rule structure, bounded rules/examples/steps/depth/frontier metadata, and strict nested unknown-field behavior.

**Step 2: Run RED**

```bash
cargo test -p amari-discovery --test wire_schema_registry --all-features \
  rewrite_contracts
```
Expected: missing or unsupported recursive tagged-enum derive.

**Step 3: Extend bounded derive only as needed**

Support the existing `#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]` recursive enum shape. Reject unrelated enum representations.

**Step 4: Run GREEN**

```bash
cargo test -p amari-discovery --all-features
```
Expected: pass.

**Step 5: Commit**

```bash
git add amari-discovery-macros amari-discovery/src/probes/rewrite.rs \
  amari-discovery/tests
git commit -m "feat: add wire contracts to rewrite probes"
```

## Task 11: Add CLI schema resolution

**Files:**
- Modify: `amari-discovery/src/cli.rs`
- Modify: `amari-discovery/src/commands/probe.rs`
- Modify: `amari-discovery/src/render.rs`
- Modify: `amari-discovery/tests/cli_probes.rs`
- Create golden fixtures if needed

**Step 1: RED CLI tests**

Add:

```bash
amari probe schema amari-probe:dual:polynomial-derivative:v1 --direction input --json
amari probe schema amari-probe:dual:polynomial-derivative:v1 --direction output --json
```

Require:

- complete document in envelope data;
- stable `$id` and hash;
- invalid direction/unknown probe typed error;
- `probe describe` contains compact input/output hashes;
- `probe list` remains compact;
- human output shows hash and schema command hint.

**Step 2: Run RED**

```bash
cargo test -p amari-discovery --test cli_probes --all-features
```
Expected: clap/parser failure for `probe schema`.

**Step 3: Implement command**

Add `ProbeCommand::Schema { probe_id, direction }`, resolve from registry, render complete document. Add additive `schema_hashes` or equivalent to `ProbeDescription` without removing fields.

**Step 4: Run GREEN**

```bash
cargo test -p amari-discovery --test cli_probes --all-features
cargo test -p amari-discovery --all-features
```
Expected: pass.

**Step 5: Commit**

```bash
git add amari-discovery/src/{cli.rs,commands/probe.rs,render.rs} \
  amari-discovery/tests/cli_probes.rs
git commit -m "feat: resolve probe wire schemas"
```

## Task 12: Integrate registry with catalog and worker contracts

**Files:**
- Modify: `amari-discovery/src/probes/{mod.rs,registry.rs,worker.rs}`
- Modify: `amari-discovery/tests/{probe_engine.rs,probe_worker_protocol.rs,catalog_integrity.rs}`

**Step 1: RED integration tests**

Require:

- direct `ProbeEngine` execution reports schema IDs and schema hashes;
- process-isolated worker reports the same contract identities as direct execution;
- descriptor mismatch is still rejected;
- saved probe result schema remains protocol v1-compatible;
- catalog hash changes only if checked-in structural/probe manifests intentionally change.

**Step 2: Run RED**

```bash
cargo test -p amari-discovery --test probe_engine --all-features
cargo test -p amari-discovery --test probe_worker_protocol --all-features
```
Expected: missing schema hash fields/registry integration.

**Step 3: Implement integration**

Add schema summaries to execution metadata additively, or keep execution unchanged and prove CLI describe/schema resolution is complete. Do not alter saved protocol schema `probe-v1.json` unless compatibility analysis proves additive.

**Step 4: Run GREEN**

Run both targeted tests and `cargo test -p amari-discovery --all-features`.

**Step 5: Commit**

```bash
git add amari-discovery/src/probes amari-discovery/tests
git commit -m "feat: align probe execution wire contracts"
```

## Task 13: Regenerate and verify deterministic catalogs

**Files:**
- Modify as generated: `amari-discovery/catalog/generated.json`
- Possibly modify: `amari-discovery/catalog/generated-wasm.json`
- Modify tests: catalog generation/package assertions for the new macro crate

**Step 1: RED catalog tests**

Update package-count/package-name assertions to require `amari-discovery-macros` and publish-order presence.

Run:
```bash
cargo test -p amari-discovery --test catalog_generation --all-features
cargo test -p amari-discovery --test catalog_packages --all-features
```
Expected: failures until generated catalog is refreshed.

**Step 2: Regenerate structural catalog**

```bash
cargo run -p amari-discovery --example generate_catalog -- .
```

**Step 3: Verify determinism**

```bash
git diff -- amari-discovery/catalog/generated.json
cargo run -p amari-discovery --example generate_catalog -- .
git diff --exit-code -- amari-discovery/catalog/generated.json
```
Expected: second regeneration is clean.

**Step 4: Run GREEN**

```bash
cargo test -p amari-discovery --all-features
```

**Step 5: Commit**

```bash
git add amari-discovery/catalog amari-discovery/tests
git commit -m "chore: catalog discovery schema authority crates"
```

## Task 14: Documentation and agent contract updates

**Files:**
- Modify: `amari-discovery/README.md`
- Modify: `docs/guide/amari-discovery.md`
- Modify: `docs/plans/2026-07-27-amari-discovery-schema-authority-design.md` status if needed
- Create/update examples under `amari-discovery/examples/` only if existing examples warrant it

**Step 1: RED documentation tests**

Add/extend tests asserting README documents:

- `amari probe schema`;
- compact hashes in `probe describe`;
- structural JSON Schema versus semantic Rust validation;
- protocol remains `amari.discovery/v1`.

**Step 2: Run RED**

```bash
cargo test -p amari-discovery --all-features readme_schema_docs
```
Expected: missing documentation content or examples.

**Step 3: Update docs**

Document one full dual schema example and explain the compatibility/drift rules.

**Step 4: Run GREEN**

```bash
cargo test -p amari-discovery --all-features
```

**Step 5: Commit**

```bash
git add amari-discovery/README.md docs/guide/amari-discovery.md docs/plans
git commit -m "docs: document probe schema authority"
```

## Task 15: Full verification and PR preparation

**Files:**
- No intentional source changes.

**Step 1: Format and normal-target Clippy**

```bash
cargo fmt --all --check
cargo clippy --workspace --all-features --lib --bins --tests --exclude amari-gpu -- -D warnings
```
Expected: pass. If the exact all-target lint exception is needed, use only the documented allow list and record why.

**Step 2: Discovery all-features**

```bash
cargo test -p amari-discovery --all-features
```
Expected: pass.

**Step 3: Workspace excluding GPU**

```bash
cargo test --workspace --exclude amari-gpu
```
Expected: pass.

**Step 4: Catalog/version/publish checks**

```bash
python3 scripts/verify-publish-order.py
python3 scripts/verify-publish-test-scope.py
./scripts/version-sync.sh verify 0.24.0
```
Expected: all pass before any version bump. Version remains 0.24.0 until a separate 0.24.1 version-bump PR.

**Step 5: Commit/push/open PR**

```bash
git status --short
git push -u origin feature/amari-schema-authority-0.24.1
gh pr create --base develop --head feature/amari-schema-authority-0.24.1 \
  --title "feat: add discovery schema authority" \
  --body-file /tmp/amari-schema-authority-impl-pr.md
```

## Acceptance criteria

1. All 13 executable probes resolve complete input/output schema documents.
2. The one known non-executable descriptor remains declarative and does not fabricate a compiled contract.
3. Every exported schema has a stable canonical SHA-256 hash.
4. `probe describe` exposes compact schema hashes; `probe schema` resolves full documents.
5. `amari.discovery/v1`, all current schema IDs, saved plans, and probe behavior remain compatible.
6. `amari-discovery-macros` is workspace/publish ordered before `amari-discovery`.
7. Catalog regeneration is deterministic.
8. All required tests and publish-scope checks pass without `amari-gpu` runtime reintroduction.
