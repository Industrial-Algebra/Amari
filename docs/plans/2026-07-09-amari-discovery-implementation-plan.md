# amari-discovery 0.24.0 Implementation Plan

> **REQUIRED SUB-SKILL:** Use the executing-plans skill to implement this plan task-by-task.

**Goal:** Build the `amari-discovery` package and installed `amari` command: an agent-first, human-friendly, read-only discovery runtime that inspects Rust and TypeScript projects, recommends applicable Amari capabilities with evidence, emits replayable integration plans, and runs bounded typed probes.

**Architecture:** A generated structural catalog plus curated semantic overlays feeds an Amari-native planner. Project inspectors produce versioned snapshots; holographic recall proposes candidates; explicit graph relationships and tropical/multi-objective costs rank them; `amari-rewrite` normalizes plans; registered probes validate assumptions. One typed protocol renders human text, JSON, and NDJSON. The package owns the `amari` binary; the umbrella crate remains a library.

**Tech Stack:** Rust 2021/stable, Clap 4, Serde/JSON/TOML, `syn`, `walkdir`, SHA-256 provenance, direct Amari crates (`amari-holographic`, `amari-network`, `amari-tropical`, `amari-optimization`, `amari-rewrite`, and probe domains), TDD with integration/golden/property-style tests.

**Repository:** `/home/elliotthall/working/industrial-algebra/amari/.worktrees/amari-discovery-0.24`

**Design:** `docs/plans/2026-07-09-amari-discovery-design.md`

**License note:** Preserve Amari's current workspace license (`MIT OR Apache-2.0`). New files should use `SPDX-License-Identifier: MIT OR Apache-2.0`; do not perform a repository-wide license change.

---

## Execution rules

- Follow strict RED → GREEN → REFACTOR for every behavior.
- Run each named test first and confirm it fails for the expected missing behavior.
- Keep target-project inspection read-only; tests must assert no target files change.
- No arbitrary shell/Rust/JavaScript execution in library code.
- Catalog references must be validated against generated structural records.
- Every public item needs rustdoc; all fallible APIs document errors.
- Commit after every task or tightly coupled pair of steps.
- Do not implement unresolved `amari-rewrite` decisions from `docs/plans/2026-07-09-amari-rewrite-0.24-decisions.md` here, except using the existing stable rewrite API.

---

### Task 1: Scaffold `amari-discovery` and transfer ownership of the `amari` binary

**Files:**
- Modify: `Cargo.toml`
- Delete: `src/main.rs`
- Create: `amari-discovery/Cargo.toml`
- Create: `amari-discovery/README.md`
- Create: `amari-discovery/src/lib.rs`
- Create: `amari-discovery/src/main.rs`
- Create: `scripts/verify-amari-binary-owner.py`
- Create: `scripts/verify-publish-order.py`
- Modify: `.github/workflows/publish.yml`

**Step 1: Add a committed metadata regression test before changing workspace ownership**

Create `scripts/verify-amari-binary-owner.py`:

```python
import json, subprocess
m = json.loads(subprocess.check_output([
    "cargo", "metadata", "--no-deps", "--format-version", "1"
]))
owners = [
    p["name"] for p in m["packages"]
    if any(t["name"] == "amari" and "bin" in t["kind"] for t in p["targets"])
]
assert owners == ["amari-discovery"], owners
```

Run:

```bash
python3 scripts/verify-amari-binary-owner.py
```

Expected: FAIL with `['amari']`.

**Step 2: Scaffold the package**

In root `Cargo.toml`:

- set `autobins = false` in root `[package]`;
- append `amari-discovery` to workspace members;
- add workspace dependencies used by discovery:

```toml
clap = { version = "4", features = ["derive"] }
serde_json = "1"
toml = "0.8"
walkdir = "2"
sha2 = "0.10"
hex = "0.4"
```

Create `amari-discovery/Cargo.toml`:

```toml
[package]
name = "amari-discovery"
version.workspace = true
authors.workspace = true
edition.workspace = true
license.workspace = true
rust-version.workspace = true
description = "Agent-first discovery and planning runtime for the Amari mathematical ecosystem"
repository = "https://github.com/Industrial-Algebra/Amari"
homepage = "https://github.com/Industrial-Algebra/Amari"
keywords = ["mathematics", "discovery", "agents", "planning", "cli"]
categories = ["mathematics", "command-line-utilities", "development-tools"]
readme = "README.md"
autobins = false
exclude = ["tests/fixtures/probe-test-worker.py"]

[lib]
name = "amari_discovery"
path = "src/lib.rs"

[[bin]]
name = "amari"
path = "src/main.rs"

[dependencies]
clap = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
toml = { workspace = true }
walkdir = { workspace = true }
sha2 = { workspace = true }
hex = { workspace = true }
syn = { workspace = true, features = ["full", "visit"] }
thiserror = { workspace = true }
amari-core = { workspace = true }
amari-tropical = { workspace = true }
amari-dual = { workspace = true, optional = true }
amari-network = { workspace = true }
amari-optimization = { workspace = true }
amari-holographic = { workspace = true }
amari-cgt = { workspace = true, optional = true }
amari-surreal = { workspace = true, optional = true }
amari-surcomplex = { workspace = true, optional = true }
amari-rewrite = { workspace = true }

[features]
default = ["standard-probes"]
standard-probes = [
    "dep:amari-dual",
    "dep:amari-cgt",
    "dep:amari-surreal",
    "dep:amari-surcomplex",
]
ai = []

[dev-dependencies]
assert_cmd = "2"
predicates = "3"
tempfile = "3"
```

Delete the root placeholder `src/main.rs`. Add `amari-discovery` to the publish array after all direct dependencies (including `amari-optimization` and `amari-rewrite`) and before root `amari`. Add `scripts/verify-publish-order.py` as a committed dependency-order regression check so the existing workflow-coverage CI remains GREEN from this first commit.

Create minimal crate docs in `lib.rs` and an empty `fn main() {}` in the new binary. Use the Amari license header.

**Step 3: Verify workspace ownership**

Run:

```bash
python3 scripts/verify-amari-binary-owner.py
cargo check -p amari-discovery
./scripts/verify-workflow-crates.sh
python3 scripts/verify-publish-order.py
cargo metadata --no-deps --format-version 1 | jq -r '.packages[] | select(.name == "amari-discovery") | .targets[] | [.name, .kind[]] | @tsv'
```

Expected: script PASS; exactly one `amari` binary owned by `amari-discovery`.

**Step 4: Commit**

```bash
git add Cargo.toml src/main.rs amari-discovery scripts/verify-amari-binary-owner.py scripts/verify-publish-order.py .github/workflows/publish.yml
git commit -m "feat: scaffold amari-discovery command"
```

---

### Task 2: Define protocol identifiers, envelopes, provenance, and errors

**Files:**
- Create: `amari-discovery/src/protocol.rs`
- Create: `amari-discovery/src/error.rs`
- Modify: `amari-discovery/src/lib.rs`
- Create: `amari-discovery/tests/protocol.rs`

**Step 1: Write failing protocol tests**

Create tests covering:

```rust
use std::str::FromStr;
use amari_discovery::{CapabilityId, Envelope, ProbeId, SchemaVersion};

#[test]
fn capability_and_probe_ids_use_stable_namespaces() {
    assert!(CapabilityId::from_str("amari:amari-tropical:paths:shortest-path").is_ok());
    assert!(CapabilityId::from_str("shortest-path").is_err());
    assert!(ProbeId::from_str("amari-probe:tropical:shortest-path:v1").is_ok());
    assert!(ProbeId::from_str("shortest-path").is_err());
}

#[test]
fn envelope_serializes_schema_and_provenance() {
    let envelope = Envelope::new(
        serde_json::json!({"ok": true}),
        amari_discovery::CatalogIdentity {
            version: "0.23.0".into(),
            hash: "fixture-hash".into(),
        },
        amari_discovery::Compatibility {
            status: "compatible".into(),
            reasons: vec![],
        },
        amari_discovery::ReplayMetadata {
            replayable: false,
            required_hashes: vec![],
            reasons: vec!["fixture response".into()],
        },
    );
    let json = serde_json::to_value(envelope).unwrap();
    assert_eq!(json["schema_version"], SchemaVersion::V1.as_str());
    assert!(json["provenance"]["tool_version"].is_string());
    assert_eq!(json["provenance"]["catalog"]["version"], "0.23.0");
    assert!(json["provenance"]["compatibility"]["status"].is_string());
    assert!(json["provenance"]["replay"]["replayable"].is_boolean());
}

#[test]
fn probe_results_report_backend_resources_hashes_and_assumptions() {
    let result = sample_probe_result();
    let json = serde_json::to_value(result).unwrap();
    for key in ["probe_id", "backend", "duration_micros", "resources",
                "catalog_hash", "input_hash", "validated_assumptions",
                "refuted_assumptions", "output"] {
        assert!(!json[key].is_null(), "missing {key}");
    }
    assert!(json.get("seed").is_some());
    assert!(json.get("project_hash").is_some());
    assert!(json["seed"].is_null());
    assert!(json["project_hash"].is_null());
}
```

Run:

```bash
cargo test -p amari-discovery --test protocol
```

Expected: FAIL because protocol types do not exist.

**Step 2: Implement minimal protocol**

Define documented newtypes/enums:

```rust
pub const SCHEMA_V1: &str = "amari.discovery/v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum SchemaVersion { V1 }

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CapabilityId(String);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CatalogIdentity {
    pub version: String,
    pub hash: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Compatibility {
    pub status: String,
    pub reasons: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReplayMetadata {
    pub replayable: bool,
    pub required_hashes: Vec<String>,
    pub reasons: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Provenance {
    pub tool_version: String,
    pub catalog: CatalogIdentity,
    pub compatibility: Compatibility,
    pub replay: ReplayMetadata,
    pub project_hash: Option<String>,
    pub input_hash: Option<String>,
    pub seed: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Envelope<T> {
    pub schema_version: String,
    pub provenance: Provenance,
    pub warnings: Vec<String>,
    pub data: T,
}
```

`CapabilityId::from_str` must require four or more colon-separated segments beginning with `amari`; `ProbeId` must require `amari-probe:<domain>:<operation>:vN`. Both return typed validation errors rather than panic. `Envelope::new` requires a `CatalogIdentity`, `Compatibility`, and `ReplayMetadata`; add RED serialization tests for replayable and non-replayable responses, proving catalog version/hash and compatibility/replay fields occur on every envelope.

Define the complete `ProbeResult` contract in this foundation task: stable `ProbeId`, backend, `duration_micros`, typed resource observations, optional seed/project hash, required catalog/input hashes, validated/refuted assumptions, warnings, and JSON output. The test-local `sample_probe_result()` represents a standalone unseeded probe, so `seed` and `project_hash` are present as `null`; separate plan/project probe tests require concrete values. Do not expose a production fixture API.

Define `DiscoveryError` with structured `kind()` and `exit_code()` methods. Error variants cover invalid ID/input, catalog corruption, inspection failure, probe unavailable/failed, limit exceeded, I/O, serialization, and internal failure.

Define evidence at protocol foundation time so all outcomes compile:

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Evidence {
    pub kind: String,
    pub summary: String,
    pub source: Option<String>,
    pub weight: f64,
}
```

Define a separate typed domain outcome:

```rust
pub enum DiscoveryOutcome<T> {
    Recommended(T),
    NoApplicableCapability { evidence: Vec<Evidence> },
    InsufficientEvidence { missing: Vec<String> },
    Blocked { reasons: Vec<String> },
}
```

Before implementation, add table-driven RED tests for every `DiscoveryError` kind/exit-code pair and every domain-outcome serialization. `NoApplicableCapability`, `InsufficientEvidence`, and `Blocked` are successful domain responses, never process errors.

**Step 3: Verify GREEN and docs**

```bash
cargo test -p amari-discovery --test protocol
cargo doc -p amari-discovery --no-deps
```

Expected: PASS without rustdoc warnings.

**Step 4: Commit**

```bash
git add amari-discovery/src amari-discovery/tests/protocol.rs
git commit -m "feat: add discovery protocol foundation"
```

---

### Task 3: Add dynamic capabilities and the first human/JSON CLI contract

**Files:**
- Create: `amari-discovery/src/capabilities.rs`
- Create: `amari-discovery/src/cli.rs`
- Create: `amari-discovery/src/render.rs`
- Modify: `amari-discovery/src/lib.rs`
- Modify: `amari-discovery/src/main.rs`
- Create: `amari-discovery/tests/cli_capabilities.rs`

**Step 1: Write failing binary tests**

```rust
use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn capabilities_json_self_describes_schema_and_exit_codes() {
    let output = Command::cargo_bin("amari").unwrap()
        .args(["capabilities", "--json"])
        .assert().success()
        .get_output().stdout.clone();
    let value: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(value["schema_version"], "amari.discovery/v1");
    assert_eq!(value["data"]["binary"], "amari");
    assert!(value["data"]["exit_codes"].is_object());
    assert!(value["data"]["host"]["os"].is_string());
    assert!(value["data"]["target"]["arch"].is_string());
    assert!(value["data"]["feature_gates"].is_array());
    assert_eq!(value["data"]["ai_adapter"]["contract_compiled"], cfg!(feature = "ai"));
    assert_eq!(value["data"]["ai_adapter"]["provider_configured"], false);
    assert_eq!(value["data"]["ai_adapter"]["executable"], false);
}

#[test]
fn capabilities_human_output_is_concise() {
    Command::cargo_bin("amari").unwrap()
        .arg("capabilities")
        .assert().success()
        .stdout(predicate::str::contains("Amari Discovery"))
        .stdout(predicate::str::contains("Project inspectors"));
}
```

Run and confirm RED.

**Step 2: Implement capabilities and CLI**

`Capabilities` includes binary/tool/protocol versions, output modes, resource limits, host OS/architecture, compilation target, known feature gates, optional AI contract/provider/executable states, and the complete exit-code map. Model each inspector/probe with explicit `known`, `available`, and `executable` state. Because the catalog lands in Task 4, this bootstrap task reports catalog status as unavailable (`version = "bootstrap"`, deterministic bootstrap hash) and an empty known-probe list—never a duplicated forecast. Task 4 updates capabilities to derive known probes/catalog identity from the embedded catalog; Tasks 8, 9, and 15 update availability/executability as implementations land.

Use Clap with exhaustive subcommands. Implement only `Capabilities` now; other command variants may return a typed “not implemented” error until their tasks, but do not advertise them as executable in `Capabilities` yet.

Ensure human and JSON renderers consume the same `Envelope<Capabilities>`.

**Step 3: Verify**

```bash
cargo test -p amari-discovery --test cli_capabilities
cargo run -p amari-discovery --bin amari -- capabilities
cargo run -p amari-discovery --bin amari -- capabilities --json | jq .
```

Expected: PASS; stdout contains no progress noise.

**Step 4: Commit**

```bash
git add amari-discovery
git commit -m "feat: add self-describing amari command"
```

---

### Task 4: Add catalog types, embedded loading, and semantic validation

**Files:**
- Create: `amari-discovery/src/catalog/mod.rs`
- Create: `amari-discovery/src/catalog/model.rs`
- Create: `amari-discovery/catalog/generated.json`
- Create: `amari-discovery/catalog/semantic/core.toml`
- Create: `amari-discovery/catalog/probes.toml`
- Create: `amari-discovery/tests/catalog_integrity.rs`
- Modify: `amari-discovery/src/capabilities.rs`
- Modify: `amari-discovery/tests/cli_capabilities.rs`
- Modify: `amari-discovery/src/lib.rs`

**Step 1: Write failing integrity tests**

Tests must require:

- embedded catalog parses;
- capability IDs are unique;
- every semantic crate/feature/symbol/example reference exists structurally, while every semantic probe reference resolves to the declarative `catalog/probes.toml` manifest;
- relationship endpoints exist;
- every semantic `ProbeDescriptor` has a stable `ProbeId`, capability ID, versioned input/output schema ID, limits/cost/determinism/side-effect declaration, and no duplicate ID;
- catalog hash is deterministic.

Example:

```rust
#[test]
fn embedded_catalog_has_unique_valid_capabilities() {
    let catalog = amari_discovery::Catalog::embedded().unwrap();
    catalog.validate().unwrap();
    assert!(!catalog.crates().is_empty());
    assert!(catalog.capabilities().len() >= 8);
    assert_eq!(catalog.content_hash(), catalog.content_hash());
}
```

Run and confirm RED.

**Step 2: Implement catalog model**

Define `StructuralCatalog`, `CrateRecord`, `ItemRecord`, `ExampleRecord`, `FeatureRecord`, `CapabilityRecord`, `CapabilityRelation`, `ProbeDescriptor`, `StabilityTier`, `CostHint`, and `Catalog`.

Use `include_str!("../../catalog/generated.json")` and declarative semantic/probe TOML includes. `Catalog::validate` returns typed errors; no `unwrap` in library code. Replace Task 3's bootstrap capability identity/list with values derived from this embedded catalog, keeping inspectors unavailable and probes non-executable.

Seed semantic capabilities and a separate declarative known-probe manifest for the representative planner domains:

- geometric product/rotations;
- tropical shortest paths/Viterbi;
- automatic differentiation;
- geometric networks;
- optimization;
- holographic retrieval/superposition (initially concept-level only; do not reference the absent `BindingAlgebra::superpose` symbol until Task 10);
- CGT/surreal/surcomplex;
- rewriting/normalization/inference.

Keep initial `generated.json` small but internally valid. Full workspace coverage is first required and tested in Task 5.

**Step 3: Verify GREEN only after all references validate**

```bash
cargo test -p amari-discovery --test catalog_integrity
```

**Step 4: Commit**

```bash
git add amari-discovery/src/catalog amari-discovery/src/lib.rs amari-discovery/src/capabilities.rs amari-discovery/catalog amari-discovery/tests/catalog_integrity.rs amari-discovery/tests/cli_capabilities.rs
git commit -m "feat: add semantic capability catalog"
```

---

### Task 5A1: Inventory workspace packages and metadata

**Files:** Create `amari-discovery/src/catalog/generator/{mod.rs,inventory.rs}`, modify `amari-discovery/src/catalog/mod.rs`, `amari-discovery/tests/catalog_packages.rs`, and a catalog workspace fixture.

**Steps:** RED-test workspace members, exclusion of discovery, inclusion of root/wasm, package name/version/description/license/targets. GREEN: deterministic TOML parsing for only these fields. Verify; commit `feat: inventory Amari catalog packages`.

---

### Task 5A2: Inventory features, dependencies, and examples

**Files:** Extend inventory; create `amari-discovery/tests/catalog_package_links.rs`.

**Steps:** RED-test feature dependency edges, optional/renamed/target dependencies, workspace inheritance, library/bin/example target classification, and deterministic ordering. GREEN: add only these relationships. Verify; commit `feat: index Amari package relationships`.

---

### Task 5B1: Build the complete local module graph

**Files:** Create `amari-discovery/src/catalog/generator/modules.rs`, `amari-discovery/tests/catalog_modules.rs`, and fixture modules.

**Steps:** RED-test external/inline modules, `#[path]`, missing files, cycles, and private/public visibility. GREEN: recursively parse the complete local module graph—including private modules needed as possible re-export sources—without yet declaring items publicly reachable. Verify; commit `feat: map Amari source modules`.

---

### Task 5B2: Resolve public export and re-export reachability

**Files:** Create `amari-discovery/src/catalog/generator/exports.rs` and `amari-discovery/tests/catalog_exports.rs`.

**Steps:** RED-test ordinary public modules, `mod private; pub use private::Type`, aliases, glob/local re-export chains, duplicate exported paths, and unresolved external exports as warnings. GREEN: compute reachability from the crate root and emit only actually exported paths. Verify; commit `feat: resolve Amari public exports`.

---

### Task 5B3: Extract normalized signatures and associated items

**Files:** Create `amari-discovery/src/catalog/generator/signatures.rs` and `amari-discovery/tests/catalog_signatures.rs`.

**Steps:** RED-test function/type/struct/enum/const signatures, generics/where clauses, inherent methods, and associated types/constants/functions. GREEN: normalize token streams deterministically while preserving meaningful bounds. Verify; commit `feat: index Amari API signatures`.

---

### Task 5B4: Extract trait and implementation relationships

**Files:** Create `amari-discovery/src/catalog/generator/traits.rs` and `amari-discovery/tests/catalog_traits.rs`.

**Steps:** RED-test supertraits, required/provided methods, associated items, direct generic `impl Trait for Type`, and relationships preserved through re-export aliases. GREEN: emit typed relationships, then assert real `BindingAlgebra`, `TermSystem`, `ParetoFront`, and WASM exports have expected signatures/relationships. Verify; commit `feat: index Amari trait relationships`.

---

### Task 5C1: Record cfg-gated public surfaces

**Files:** Create `amari-discovery/src/catalog/generator/cfg.rs`, `amari-discovery/tests/catalog_cfg.rs`, and cfg fixture cases.

**Steps:** RED-test simple feature gates, `all`/`any` combinations, default/disabled status, and unsupported expressions becoming `unknown_cfg`. GREEN: attach conservative normalized gates. Verify; commit `feat: index gated Amari APIs`.

---

### Task 5C2: Record exported declarative and procedural macros

**Files:** Create `amari-discovery/src/catalog/generator/macros.rs`, `amari-discovery/tests/catalog_macros.rs`, and macro/proc-macro fixtures.

**Steps:** RED-test private macro exclusion, `#[macro_export]`, exported macro re-exports, and proc-macro/proc-macro-derive/attribute functions with signatures. GREEN: implement only macro extraction. Verify; commit `feat: index Amari macros`.

---

### Task 5C3: Ingest the authoritative generated WASM/TypeScript surface

**Files:** Create `amari-discovery/src/catalog/generator/wasm.rs`, `amari-discovery/tests/catalog_wasm.rs`, `scripts/generate-discovery-wasm-surface.sh`, and checked-in `amari-discovery/catalog/generated-wasm.json`.

**Steps:**

1. RED fixture tests parse wasm-bindgen `.d.ts` classes/methods/aliases and map them to Rust/shared capability IDs.
2. RED real-workspace assertions require generated exports including `WasmMultivector300.geometricProduct`, arbitrary-signature classes, and convenience aliases.
3. GREEN: CI installs `wasm32-unknown-unknown` plus `wasm-pack`; the contributor script runs `wasm-pack build` into a temp directory, parses the authoritative `.d.ts`, writes only fixed `catalog/generated-wasm.json`, and removes temp output. Runtime generation remains pure/read-only.
4. Add CI setup/check for wasm-pack artifact drift; verify; commit `feat: index generated Amari WASM APIs`.

---

### Task 5D: Generate deterministic snapshot and enforce CI drift

**Files:**
- Create: `amari-discovery/examples/generate_catalog.rs`
- Replace: `amari-discovery/catalog/generated.json`
- Create: `amari-discovery/tests/catalog_generation.rs`
- Modify: `.github/workflows/ci.yml`

**Steps:**

1. RED: require deterministic sorted output/hash, second-generation equality, semantic-reference validation, and checked-in snapshot equality.
2. Implement pure library generation/verification only:

```rust
pub fn generate_workspace_catalog(root: &Path) -> DiscoveryResult<StructuralCatalog>;
pub fn verify_checked_in(root: &Path, checked_in: &Path) -> DiscoveryResult<()>;
```

The contributor-only example canonicalizes the supplied workspace root, requires the `amari-discovery` package, and writes only to fixed `<root>/amari-discovery/catalog/generated.json`; it accepts no output path and is not exposed by the installed `amari` command. Exclude `target`, `.git`, `.worktrees`, and `amari-discovery` itself. Known probe descriptors come from the declarative Task 4 `catalog/probes.toml` manifest. Task 5D incorporates that manifest into generated metadata while still excluding discovery's own public Rust API; Task 15 validates executable adapters against these generated descriptor records.
3. Generate twice and verify:

```bash
cargo run -p amari-discovery --example generate_catalog -- .
cargo test -p amari-discovery --test catalog_generation --test catalog_integrity
```

4. Add the drift test to `.github/workflows/ci.yml`; commit `feat: generate validated Amari API catalog`.

---

### Task 6: Add progressive catalog discovery commands

**Files:**
- Create: `amari-discovery/src/commands/discover.rs`
- Create: `amari-discovery/src/commands/mod.rs`
- Modify: `amari-discovery/src/cli.rs`
- Modify: `amari-discovery/src/main.rs`
- Create: `amari-discovery/tests/cli_discover.rs`

**Step 1: Write failing CLI tests**

Cover:

```text
amari discover search tropical --json
amari discover detail amari:amari-tropical:paths:shortest-path --json
amari discover graph <id> --json
amari discover example <id> --json
```

Assert progressive output: search records are compact; detail is complete; graph relationships refer to valid IDs; unknown IDs produce structured stderr and nonzero documented exit code.

**Step 2: Implement catalog queries**

Search name, aliases, concepts, descriptions, crate/module/symbol names. Rank exact ID/name first, then prefix, then semantic/substring match. Deterministic tie-break by capability ID.

All command handlers return typed envelopes; rendering stays centralized.

**Step 3: Verify**

```bash
cargo test -p amari-discovery --test cli_discover
cargo run -p amari-discovery --bin amari -- discover search tropical --json | jq '.data.results'
```

**Step 4: Commit**

```bash
git add amari-discovery
git commit -m "feat: add progressive capability discovery"
```

---

### Task 7: Add bounded, read-only project traversal and snapshots

**Files:**
- Create: `amari-discovery/src/inspect/mod.rs`
- Create: `amari-discovery/src/inspect/limits.rs`
- Create: `amari-discovery/src/inspect/snapshot.rs`
- Create: `amari-discovery/tests/inspection_safety.rs`
- Modify: `amari-discovery/src/lib.rs`

**Step 1: Write failing safety tests**

Use `tempfile` fixtures to assert:

- project hash is stable;
- traversal ignores `.git`, `target`, `node_modules`, and `.worktrees`;
- symlinks leaving the project root are not followed;
- files above configured size are skipped with warnings;
- file-count/time limits return partial evidence plus `LimitExceeded` state;
- snapshots contain extracted signals/locations/hashes rather than full source text or environment secrets;
- target file content, permissions, size, and modification time are identical before and after inspection (do not assert access time).

**Step 2: Implement common inspector**

Define `InspectionLimits`, `ProjectKind`, `ProjectSignal`, `SourceLocation`, `ProjectSnapshot`, and `ProjectInspector` trait.

Default limits must be surfaced by `amari capabilities`. Hash only the bounded inspected inputs, using sorted relative paths and SHA-256.

**Step 3: Verify**

```bash
cargo test -p amari-discovery --test inspection_safety
```

Expected: PASS with no changed fixture files.

**Step 4: Commit**

```bash
git add amari-discovery/src/inspect amari-discovery/src/lib.rs amari-discovery/tests/inspection_safety.rs
git commit -m "feat: add safe project inspection core"
```

---

### Task 8A: Resolve Cargo manifests, workspace inheritance, and lockfile versions

**Files:**
- Create: `amari-discovery/src/inspect/cargo.rs`
- Create: `amari-discovery/tests/fixtures/rust-project/Cargo.toml.in`
- Create: `amari-discovery/tests/fixtures/rust-project-stale/Cargo.toml`
- Create: `amari-discovery/tests/cargo_inspection.rs`

**Steps:**

1. RED fixtures cover root `amari = "<current>"` with no nonexistent `tropical` feature (the library re-exports tropical unconditionally), direct/renamed crates, workspace inheritance, target-specific dependencies, `[[bench]]` declarations, native `links`/system dependency signals, current lockfile resolution, and intentionally stale 0.19.0.
2. At test time, copy templates to `TempDir` and substitute the current workspace version.
3. GREEN: parse manifests/workspace/target tables and `Cargo.lock` without invoking Cargo/network. Report declared and resolved versions; exact catalog match is applicable, mismatch is `unknown_version`.
4. Verify; commit `feat: resolve Cargo Amari dependencies`.

---

### Task 8B: Extract Rust source usage and project vocabulary

**Files:**
- Create: `amari-discovery/src/inspect/rust.rs`
- Create: `amari-discovery/tests/fixtures/rust-project/src/lib.rs`
- Create: `amari-discovery/tests/fixtures/rust-project/README.md`
- Create: `amari-discovery/tests/rust_source_inspection.rs`

**Steps:**

1. RED: imports through `amari::tropical`, direct/renamed crates, cfgs, tests/examples/benches, `#![no_std]` and other crate attributes, WASM/native-link vocabulary, docs/comments, source locations, malformed source warnings, and unrelated-source invariance.
2. GREEN: parse with `syn`, bounded vocabulary extraction, and no full-source persistence.
3. Verify; commit `feat: inspect Rust usage of Amari APIs`.

---

### Task 8B2: Inspect Cargo target and platform configuration

**Files:** Create `amari-discovery/src/inspect/cargo_config.rs`, `.cargo/config.toml` and bench fixtures, and `amari-discovery/tests/cargo_platform_inspection.rs`.

**Steps:** RED-test bounded parsing of `.cargo/config.toml` build/target/rustflags/runner settings, configured WASM targets, native linker requirements, `benches/` plus `[[bench]]`, `no_std`, target cfg constraints, and missing/malformed config warnings. GREEN: collect typed platform/benchmark evidence without invoking Cargo, runners, linkers, or build scripts. Verify; commit `feat: inspect Rust platform constraints`.

---

### Task 8C: Expose Rust inspection through `amari inspect`

**Files:**
- Modify: `amari-discovery/src/inspect/mod.rs`
- Modify: `amari-discovery/src/capabilities.rs`
- Modify: `amari-discovery/tests/cli_capabilities.rs`
- Create: `amari-discovery/tests/rust_inspection.rs`

**Steps:**

1. RED: integration test materializes a current-version fixture into `TempDir`, invokes `CARGO_BIN_EXE_amari inspect <temp> --json`, and asserts dependency/source/domain/benchmark/platform evidence; a stale fixture reports `unknown_version`.
2. GREEN: compose manifest and source inspectors; human output summarizes usage/warnings; JSON emits `Envelope<ProjectSnapshot>`.
3. Run `cargo test -p amari-discovery --test rust_inspection --test cli_capabilities`; commit `feat: expose Rust project inspection`.

---

### Task 9A: Resolve npm manifests and lockfile versions

**Files:** Create npm inspector, current/stale package templates, and `amari-discovery/tests/npm_packages.rs`.

**Steps:** Scope v0.24 explicitly to npm `package.json` plus supported `package-lock.json` schema versions; capabilities advertise `npm-typescript`, not Yarn/pnpm. RED-test declared/lockfile-resolved `@justinelliottcobb/amari-wasm`, current-version materialization, stale `unknown_version`, missing/malformed files, and supported npm lockfile versions. GREEN: parse bounded JSON only; never invoke npm/node. Verify; commit `feat: resolve Amari WASM packages`.

---

### Task 9B: Extract TypeScript imports, d.ts exports, and runtime signals

**Files:** Modify npm inspector; create TS/d.ts fixtures and `amari-discovery/tests/ts_source_inspection.rs`.

**Steps:** RED-test imports, aliases, generated d.ts exports, bundler/runtime signals, missing d.ts warnings, capability-ID mapping, limits, and unrelated-source invariance. GREEN: conservative bounded scanners only. Verify; commit `feat: inspect TypeScript Amari usage`.

---

### Task 9C: Expose TypeScript through shared `amari inspect`

**Files:** Modify inspect dispatch/capabilities and create `amari-discovery/tests/npm_inspection.rs`.

**Steps:** RED: materialize current/stale TS fixtures, invoke `CARGO_BIN_EXE_amari inspect <temp> --json`, assert npm/source/capability evidence, human/JSON parity, and inspector availability in capabilities. GREEN: dispatch by project evidence without invoking external tools. Run npm/capability tests; commit `feat: expose TypeScript project inspection`.

---

### Task 10: Canonically implement additive `BindingAlgebra::superpose` and `scale`

**Files:**
- Modify: `amari-holographic/src/algebra/mod.rs`
- Modify: concrete algebra implementations only when optimized overrides are warranted
- Create: `amari-holographic/tests/superposition.rs`
- Update: `amari-holographic/README.md`
- Modify: `amari-discovery/catalog/semantic/core.toml`

**Ownership precondition:** PR #176 is the design/handoff source. Before coding, fetch its status. This feature branch is designated the canonical code implementation: rebase if #176's docs merged, reuse its design text, and do not duplicate/cherry-pick a competing implementation. Update/close the docs-only PR as superseded by the implementation PR when appropriate. For v0.24, documented trait defaults are canonical; concrete overrides are added only when benchmarks demonstrate a need, with parity tests, rather than copied automatically from the handoff.

**Step 1: Write failing trait tests**

Test default behavior using a small test algebra implemented through existing coefficient methods:

- `superpose` is coefficient-wise addition;
- `scale` is coefficient-wise scalar multiplication;
- superposition is commutative and has zero identity;
- repeated superposition magnitude grows while `bundle` remains normalized/attention-like;
- the default trait implementation agrees with explicit coefficient reconstruction for MAP and `CliffordAlgebra` (`to_coefficients` → coefficient-wise operation → `from_coefficients`). Use UFCS for shadowed trait methods, e.g. `<MAP256 as BindingAlgebra>::scale(&value, scalar)`. Add override-parity tests only if an optimized concrete override is actually introduced.

Run:

```bash
cargo test -p amari-holographic --test superposition
```

Expected: FAIL because methods do not exist.

**Step 2: Implement documented defaults**

Add to `BindingAlgebra`:

```rust
fn superpose(&self, other: &Self) -> AlgebraResult<Self> {
    if self.dimension() != other.dimension() {
        return Err(AlgebraError::DimensionMismatch { /* existing fields */ });
    }
    let coeffs: Vec<f64> = self.to_coefficients().into_iter()
        .zip(other.to_coefficients())
        .map(|(a, b)| a + b)
        .collect();
    Self::from_coefficients(&coeffs)
}

fn scale(&self, scalar: f64) -> AlgebraResult<Self> {
    let coeffs: Vec<f64> = self.to_coefficients().into_iter()
        .map(|value| value * scalar)
        .collect();
    Self::from_coefficients(&coeffs)
}
```

Use the actual existing `AlgebraError` field shape. Document that `bundle` is attention/cleanup and `superpose` is additive accumulation.

**Step 3: Verify**

```bash
cargo test -p amari-holographic --test superposition
cargo test -p amari-holographic --quiet
cargo clippy -p amari-holographic --all-targets -- -D warnings
cargo run -p amari-discovery --example generate_catalog -- .
cargo test -p amari-discovery --test catalog_generation --test catalog_integrity
```

Update the holographic semantic capability/probe records to reference the now-existing `BindingAlgebra::superpose`/`scale` symbols. Regenerating here is mandatory because those methods change the public structural catalog.

**Step 4: Commit**

```bash
git add amari-holographic amari-discovery/catalog/generated.json amari-discovery/catalog/semantic/core.toml
git commit -m "feat: add additive holographic superposition"
```

---

### Task 11: Add deterministic candidate retrieval with holographic recall

**Files:**
- Create: `amari-discovery/src/planner/mod.rs`
- Create: `amari-discovery/src/planner/recall.rs`
- Create: `amari-discovery/tests/planner_recall.rs`
- Modify: `amari-discovery/src/lib.rs`

**Step 1: Write failing recall tests**

Assert:

- exact concepts retrieve expected capability first;
- related vocabulary retrieves a non-literal capability (e.g. “routing under path costs” → tropical/network path capability);
- same seed/catalog/snapshot yields byte-identical ranking;
- holographic recall only returns catalog IDs;
- lexical fallback still works if holographic confidence is below threshold.

**Step 2: Implement retrieval**

Define `CandidateRetriever` and `RetrievedCandidate`. Use deterministic token-to-seed hashing and `MAPAlgebra`/holographic operations. Build capability vectors using additive `superpose`, then normalize for similarity. Do not use `bundle` for accumulation.

Return score components and matched evidence. Holographic score is a candidate-generation signal, not final confidence.

**Step 3: Verify**

```bash
cargo test -p amari-discovery --test planner_recall
```

**Step 4: Commit**

```bash
git add amari-discovery/src/planner amari-discovery/src/lib.rs amari-discovery/tests/planner_recall.rs
git commit -m "feat: add holographic capability recall"
```

---

### Task 12A: Expand candidates through the bounded capability graph

**Files:**
- Create: `amari-discovery/src/planner/graph.rs`
- Modify: `amari-discovery/src/planner/mod.rs`
- Create: `amari-discovery/tests/planner_graph.rs`

**Steps:**

1. RED: prerequisites/composition edges expand candidates; invalidating constraints block them; node/depth limits preserve partial paths; edge costs must be finite and nonnegative.
2. RED: compare graph reachability/path selection with `amari_network::GeometricNetwork::shortest_path`; separately prove path-edge accumulation/comparison parity with `amari_tropical::verified::{MinPlus, VerifiedTropicalNumber}` (`tropical_mul` accumulates edge costs; `tropical_add` selects the minimum). Avoid `TropicalNetwork::shortest_path_tropical`.
3. GREEN: build the typed capability graph with deterministic ID ordering, use `GeometricNetwork` for relationships/path reconstruction, and use the min-plus values for path-cost accumulation/comparison.
4. Verify focused tests; commit `feat: expand capability integration graph`.

---

### Task 12B: Rank graph candidates with transparent Pareto trade-offs

**Files:**
- Create: `amari-discovery/src/planner/rank.rs`
- Create: `amari-discovery/tests/planner_ranking.rs`

**Steps:**

1. RED: define applicability, evidence, effort, maturity, runtime, platform, verification, and risk components; require a documented canonical all-minimization vector (negate benefit dimensions).
2. RED: satisfying prerequisites improves/unblocks the same ID; removing evidence never increases confidence; irrelevant signals do not reorder; Pareto alternatives survive; preferred tie-break is deterministic; validated saved `ProbeResult`s with matching provenance improve verification/confidence, refuted assumptions block or demote the same ID, and mismatched provenance is ignored with warnings.
3. GREEN: adapt candidates to `amari_optimization::multiobjective::ParetoFront`, preserve component/evidence breakdowns, and report any deterministic fallback in provenance.
4. Run `cargo test -p amari-discovery --test planner_ranking`; commit `feat: rank capability integration paths`.

---

### Task 13: Normalize replayable plans with `amari-rewrite`

**Files:**
- Create: `amari-discovery/src/planner/plan.rs`
- Create: `amari-discovery/src/planner/normalize.rs`
- Modify: `amari-discovery/src/protocol.rs`
- Create: `amari-discovery/tests/plan_normalization.rs`

**Step 1: Write failing plan tests**

Assert:

- duplicate feature/dependency steps normalize to one;
- prerequisite ordering is canonical;
- normalization has a bounded trace;
- plan contains exact crates/features/symbols/examples/probes/tests;
- project/catalog/input hashes are present;
- incompatible project/catalog hash prevents replay with a typed drift error;
- repeated normalization is idempotent.

**Step 2: Implement plan term encoding and rewrite rules**

Encode plan steps into discovery-local `amari_rewrite::trs::Term` values. Implement a discovery-local bounded loop over public `amari_rewrite::trs::TermSystem::apply_once` and `amari_rewrite::trs::Rule` APIs, recording each before/after step as a `NormalizationTrace`; do not assume the current rewrite crate exposes tracing or serde impls. Decode the final term back into plan steps. Keep rules explicit and inspectable.

Add `GoalSpec`, `PlanningContext { snapshot, goal, probe_results }`, `Recommendation`, `CandidatePlan`, `PlanStep`, and compatibility metadata to the protocol; extend the existing Task 2 `Evidence` type only additively if planning needs more structured fields.

**Step 3: Verify**

```bash
cargo test -p amari-discovery --test plan_normalization
```

**Step 4: Commit**

```bash
git add amari-discovery
git commit -m "feat: generate replayable normalized plans"
```

---

### Task 14A: Add `recommend` for the Rust project vertical slice

**Files:** Create `amari-discovery/src/commands/recommend.rs`, modify commands/CLI, and create `amari-discovery/tests/cli_recommend_rust.rs`.

**Steps:** RED-test a materialized Rust fixture with inline `--goal`, optional `--probe-results <file>`, preferred candidate/alternatives/scores/evidence/missing info/probes/tests, fixed-seed determinism, human/JSON parity, and no mutation. GREEN: wire inspect→goal→recall→graph→rank for Rust and render typed output. Verify; commit `feat: recommend Amari integrations for Rust projects`.

---

### Task 14B: Add TypeScript recommendation parity and goal-file input

**Files:** Modify recommend/CLI; create `amari-discovery/tests/cli_recommend_ts.rs` and goal fixtures.

**Steps:** RED-test TS project parity, `--goal-file`, optional saved probe-result input, mutual exclusion with `--goal`, malformed goal errors, and current/stale WASM versions. GREEN: reuse the same typed pipeline with npm snapshots. Verify; commit `feat: recommend Amari integrations for TypeScript projects`.

---

### Task 14C: Add saved recommendation → plan replay

**Files:** Create `amari-discovery/src/commands/plan.rs`; modify command routing; create `amari-discovery/tests/cli_plan_replay.rs`.

**Steps:** RED-test saving recommendation JSON, selecting candidate in a fresh process, snapshot/catalog/input/probe-result hash validation, normalized plan/provenance parity with in-process planning, unknown candidate, and changed-project drift. GREEN: require `--recommendation` and current project path, validate artifact compatibility, select/normalize candidate, and render. Verify; commit `feat: replay discovery recommendations as plans`.

---

### Task 15: Add the typed probe registry and one tropical proof slice

**Files:**
- Create: `amari-discovery/src/probes/mod.rs`
- Create: `amari-discovery/src/probes/registry.rs`
- Create: `amari-discovery/src/probes/tropical.rs`
- Create: `amari-discovery/tests/probe_engine.rs`
- Add: private registry-construction/rejection unit tests in `amari-discovery/src/probes/registry.rs`
- Create: `amari-discovery/tests/probe_tropical.rs`
- Modify: `amari-discovery/src/capabilities.rs`
- Modify: `amari-discovery/tests/cli_capabilities.rs`
- Modify: `amari-discovery/src/lib.rs`

**Steps:**

1. Write RED registry tests requiring each executable adapter to map one-to-one to a known semantic `ProbeDescriptor` from Task 4. Reject duplicate adapters, unknown descriptor IDs, capability mismatches, network/side-effect declarations, and schema/limit/determinism/feature mismatches.
2. Write a RED `TropicalViterbiRequest { transitions, emissions, observations }` parity test against `amari_tropical::viterbi::TropicalViterbi::decode`, including matrix-shape, state/observation, request-byte, and output-byte ceilings. Do not use `TropicalNetwork::shortest_path_tropical`, whose current implementation/tests are unsuitable for this contract.
3. Keep `ProbeRegistry` and adapters crate-private. Expose a public `ProbeEngine` that validates input and executes in-process under cooperative request/node/iteration/byte limits, reporting `isolation = "cooperative"`; it cannot promise crash or wall-clock isolation. Implement the single Viterbi proof-slice adapter. Register executable domain probes only under `standard-probes`; a `--no-default-features` build still knows catalog capabilities but reports probes as non-executable.
4. Derive executable probe state in `Capabilities` from the registry; do not duplicate names.
5. Run:

```bash
cargo test -p amari-discovery --lib probes::registry::tests
cargo test -p amari-discovery --test probe_engine --test probe_tropical --test cli_capabilities
cargo test -p amari-discovery --no-default-features --test probe_engine --test cli_capabilities
cargo check -p amari-discovery --no-default-features
```

6. Commit:

```bash
git add amari-discovery
git commit -m "feat: add typed probe registry"
```

---

### Task 16A: Add the core geometric algebra probe

**Files:**
- Create: `amari-discovery/src/probes/core.rs`
- Create: `amari-discovery/tests/probe_core.rs`
- Modify: `amari-discovery/src/probes/mod.rs`
- Modify: `amari-discovery/src/probes/registry.rs`
- Modify: private registry unit tests in `amari-discovery/src/probes/registry.rs`
- Modify: `amari-discovery/tests/cli_capabilities.rs`

**Steps:**

1. RED through public `ProbeEngine`: define `Cl3ProductRequest { left: [f64; 8], right: [f64; 8] }` and output coefficients; require direct `Multivector<3,0,0>::geometric_product` parity, identity, finite-number validation, limits, and a new assertion that the known descriptor is now executable (prior capability tests already establish the pre-task non-executable baseline).
2. GREEN: declare/register the adapter behind `standard-probes` in the same slice, update registry/capabilities, and implement.
3. Run core/registry/capability tests; commit `feat: add core discovery probe`.

---

### Task 16B: Add the dual-number probe

**Files:** Create dual adapter/test; modify probe module/registry and registry/capability tests.

**Steps:** RED through public `ProbeEngine`: bounded `PolynomialDerivativeRequest`, direct dual Horner value/derivative parity, empty/non-finite/limit errors, and a new assertion that the known descriptor is now executable (prior capability tests already establish the pre-task non-executable baseline). GREEN: register behind `standard-probes` in the same slice and implement. Verify; commit `feat: add dual discovery probe`.

---

### Task 17A: Add the network shortest-path probe

**Files:**
- Create: `amari-discovery/src/probes/network.rs`
- Create: `amari-discovery/tests/probe_network.rs`
- Modify: `amari-discovery/src/probes/mod.rs`
- Modify: `amari-discovery/src/probes/registry.rs`
- Modify: private registry unit tests in `amari-discovery/src/probes/registry.rs`
- Modify: `amari-discovery/tests/cli_capabilities.rs`

**Steps:**

1. RED through `ProbeEngine`: bounded adjacency request, deterministic positions, direct `GeometricNetwork::shortest_path` parity, shape/weight/node/index/unreachable cases, and a new assertion that the known descriptor is now executable (prior capability tests already establish the pre-task non-executable baseline).
2. GREEN: register behind `standard-probes` in this slice and implement.
3. Verify network/registry/capability tests; commit `feat: add network discovery probe`.

---

### Task 17B: Add the Pareto optimization probe

**Files:** Create optimization adapter/test; modify probe module/registry and registry/capability tests.

**Steps:** RED through `ProbeEngine`: objective/direction DTO, maximize-to-minimize transformation, direct `Individual`/`ParetoFront` parity, dimension/population limits, deterministic order, and a new assertion that the known descriptor is now executable (prior capability tests already establish the pre-task non-executable baseline). GREEN: register behind `standard-probes` in this slice and implement. Verify; commit `feat: add optimization discovery probe`.

---

### Task 18A: Add the holographic superposition probe

**Files:**
- Create: `amari-discovery/src/probes/holographic.rs`
- Create: `amari-discovery/tests/probe_holographic.rs`
- Modify: `amari-discovery/src/probes/mod.rs`
- Modify: `amari-discovery/src/probes/registry.rs`
- Modify: private registry unit tests in `amari-discovery/src/probes/registry.rs`
- Modify: `amari-discovery/tests/cli_capabilities.rs`

**Steps:**

1. RED through `ProbeEngine`: deterministic `MAP256` seeds, repeated `BindingAlgebra::superpose` parity, distinction from `bundle`, empty input, limits, and a new assertion that the known descriptor is now executable (prior capability tests already establish the pre-task non-executable baseline).
2. GREEN: register behind `standard-probes` in this slice and implement.
3. Verify holographic/registry/capability tests; commit `feat: add holographic superposition probe`.

---

### Task 18B: Add the holographic recall probe

**Scope note:** This adapter parity-tests existing `HolographicMemory` storage/retrieval semantics only. Discovery candidate accumulation uses Task 18A's explicit repeated `superpose` path; existing `HolographicMemory::store` bundle behavior cannot reintroduce attention-style accumulation.

**Files:** Modify holographic adapter/test and probe registry/capability tests.

**Steps:** RED through `ProbeEngine`: bounded entries, direct `HolographicMemory<MAP256>` retrieval parity, deterministic confidence, capacity warnings, and a new assertion that the known descriptor is now executable (prior capability tests already establish the pre-task non-executable baseline). GREEN: register behind `standard-probes` in this slice and implement. Verify; commit `feat: add holographic recall probe`.

---

### Task 19A: Add the bounded CGT nim-sum probe

**Files:**
- Create: `amari-discovery/src/probes/cgt.rs`
- Create: `amari-discovery/tests/probe_cgt.rs`
- Modify: `amari-discovery/src/probes/mod.rs`
- Modify: `amari-discovery/src/probes/registry.rs`
- Modify: private registry unit tests in `amari-discovery/src/probes/registry.rs`
- Modify: `amari-discovery/tests/cli_capabilities.rs`

**Steps:**

1. RED through `ProbeEngine`: heap count/value/checked option-entry bounds before allocation, direct per-heap `GameArena::grundy`, XOR result, boundaries/overflow, and a new assertion that the known descriptor is now executable (prior capability tests already establish the pre-task non-executable baseline); never call recursive `GameArena::add`.
2. GREEN: register behind `standard-probes` in this slice and implement.
3. Verify CGT/registry/capability tests; commit `feat: add cgt discovery probe`.

---

### Task 19B: Add rational-surreal arithmetic probe

**Files:** Create surreal adapter/test; modify probe module/registry and registry/capability tests.

**Steps:** The prior capability test establishes known-but-non-executable baseline. New RED tests expect successful `ProbeEngine` execution, exact API parity, bounded decimal `i128` parsing, overflow/length/zero-denominator errors, and `executable = true`; they initially fail with unavailable/false. GREEN: register in this slice and implement without changing the tests. Verify; commit `feat: add rational surreal discovery probe`.

---

### Task 19C: Add rational-surcomplex division probe

**Files:** Modify surreal adapter/test and registry/capability tests.

**Steps:** The prior capability test establishes known-but-non-executable baseline. New RED tests expect successful `ProbeEngine` execution, exact `1/(1+1/2i)=4/5-2/5i`, zero-division/bounded-input errors, and `executable = true`; they initially fail with unavailable/false. GREEN: register in this slice and implement without changing the tests. Verify; commit `feat: add surcomplex discovery probe`.

---

### Task 20A: Add bounded rewrite DTO conversion and growth analysis

**Files:** Create `amari-discovery/src/probes/rewrite.rs` and modify `amari-discovery/src/probes/mod.rs`; keep private DTO/growth tests as `#[cfg(test)]` unit tests in `rewrite.rs`.

**Steps:** RED-test recursive term/rule DTO conversion, request bytes, term depth/nodes, rule count, encoded output bytes, duplicate RHS variables, checked constant-growth bounds, and overflow. GREEN: implement validation/conversion only; verify; commit `feat: validate bounded rewrite probe inputs`.

---

### Task 20B: Add bounded rewrite normalization probe

**Files:** Modify rewrite probe/module/registry and registry/capability tests; create `amari-discovery/tests/probe_rewrite_normalize.rs`.

**Steps:** RED through public `ProbeEngine`: direct one-step parity, invalid/expanding rule rejection, step/node/depth/byte exhaustion, and a new assertion that the known descriptor is now executable (prior capability tests already establish the pre-task non-executable baseline). GREEN: register behind `standard-probes` in this slice and implement the checked `TermSystem::apply_once` loop. Verify; commit `feat: add rewrite normalization probe`.

---

### Task 20C: Add bounded predecessor-search probe

**Files:** Modify rewrite probe/module/registry and registry/capability tests; create `amari-discovery/tests/probe_rewrite_predecessors.rs`.

**Steps:** RED through public `ProbeEngine`: bounded predecessor semantics, frontier/node/depth/byte limits, deduplication, deterministic order, and a new assertion that the known descriptor is now executable (prior capability tests already establish the pre-task non-executable baseline). GREEN: register behind `standard-probes` in this slice and implement checked frontier expansion; do not collect an unbounded iterator. Verify; commit `feat: add rewrite predecessor probe`.

---

### Task 20D: Add inference probe and register rewrite adapters

**Files:** Modify rewrite probe, `probes/mod.rs`, `probes/registry.rs`, registry/capability tests; create `amari-discovery/tests/probe_rewrite_infer.rs`.

**Steps:** RED through public `ProbeEngine`: `infer_rule` parity, empty/oversized examples, generated rule limits, descriptor mapping, capabilities, and a new assertion that the known descriptor is now executable (prior capability tests already establish the pre-task non-executable baseline). GREEN: implement and register the inference adapter behind `standard-probes` in this slice. Run focused rewrite/registry/capability tests; commit `feat: add rewrite inference probes`.

---

### Task 21A: Define bounded probe worker framing and child behavior

**Files:**
- Create: `amari-discovery/src/probes/worker.rs`
- Modify: `amari-discovery/src/main.rs`
- Create: `amari-discovery/tests/probe_worker_protocol.rs`
- Modify: `amari-discovery/src/probes/mod.rs`

**Steps:**

1. RED: bounded length-prefixed request/response codec, malformed/truncated-frame rejection, a typed worker request containing only probe ID/input/limits/provenance (no project paths/handles or executable names/arguments), and registry-only dispatch rejecting descriptors with network/side effects.
2. GREEN: implement the codec and hidden fixed `__probe-worker` mode receiving only the typed probe request and invoking only the in-process registry.
3. Verify focused tests; commit `feat: add bounded probe worker protocol`.

---

### Task 21B1: Restrict worker launching and child context

**Files:** Create `amari-discovery/src/probes/supervisor.rs`, modify `amari-discovery/src/probes/mod.rs`, `amari-discovery/tests/fixtures/probe-test-worker.py`, and unit tests in the supervisor module.

**Steps:** RED-test production command/args are fixed, no shell/config/input can select executables, environment is allowlisted, CWD is neutral, and no project context is passed. GREEN: implement private `WorkerLauncher` with production/test launchers. Verify; commit `feat: restrict probe worker launch`.

---

### Task 21B2: Drain bounded stdout/stderr without deadlock

**Files:** Modify supervisor; add focused `#[cfg(test)]` unit tests in `supervisor.rs` using the private injectable launcher.

**Steps:** RED-test simultaneous stdout/stderr, output flooding, caps, and valid frame decode using the fixture. GREEN: concurrently drain bounded streams and terminate on cap violation. Verify; commit `feat: bound probe worker IO`.

---

### Task 21B3: Enforce timeout, kill, and reap semantics

**Files:** Modify supervisor; add focused private-launcher unit tests in `supervisor.rs`.

**Steps:** RED-test slow child deadline, kill, wait/reap, and no orphan; GREEN: implement deadline supervision and reliable cleanup. Verify; commit `feat: enforce probe deadlines`.

---

### Task 21B4: Map crash and exit outcomes with provenance

**Files:** Modify supervisor/error; add focused private-launcher unit tests in `supervisor.rs`.

**Steps:** RED-test crash, abort/nonzero, malformed result, and success preserving Task 2 provenance. GREEN: implement typed outcome mapping. Verify; commit `feat: report probe worker outcomes`.

---

### Task 21C: Expose the probe CLI

**Files:**
- Create: `amari-discovery/src/commands/probe.rs`
- Modify: `amari-discovery/src/cli.rs`
- Create: `amari-discovery/tests/cli_probes.rs`

**Steps:**

1. RED: `list`, `describe`, `run --dry-run --plan`, and `run --input` in human/JSON modes; structured errors; plan-based dry-run never starts a worker; `run --plan` without `--dry-run` is rejected with guidance to provide explicit typed input.
2. GREEN: CLI explicit-input execution always routes through the private supervisor/registry worker (`isolation = "process"`); plan suggestions use compatibility-only dry-run. The public library `ProbeEngine` remains cooperative in-process. Add parity tests proving both paths return identical validated mathematical output/provenance fields while explicitly differing in isolation level and hard timeout/crash guarantees. Derive capability executability dynamically.
3. Validate descriptors/catalog and binary output:

```bash
cargo test -p amari-discovery --test cli_probes --test cli_capabilities --test probe_engine
cargo test -p amari-discovery --test catalog_generation --test catalog_integrity
```

4. Commit `feat: expose isolated Amari capability probes`.

---

### Task 22A: Add five versioned protocol schemas

**Files:** Create `amari-discovery/src/schema.rs`, `amari-discovery/tests/schema_contract.rs`, and schema goldens.

**Steps:** RED-test request/response/goal/plan/probe schema `$id`, `amari.discovery/v1`, required fields, and golden compatibility. GREEN: emit curated schemas (use schemars only if it reduces duplication). Verify; commit `feat: add discovery protocol schemas`.

---

### Task 22B: Add the shared NDJSON renderer and framing

**Files:** Create `amari-discovery/src/ndjson.rs` and `amari-discovery/tests/ndjson.rs`.

**Steps:** RED-test one complete object per line, escaping/newlines, bounded record size, flush/error behavior, and envelope provenance. GREEN: implement renderer independent of commands. Verify; commit `feat: add discovery NDJSON output`.

---

### Task 22C: Integrate schema/NDJSON across command families

**Files:** Modify CLI/render/error modules; create `amari-discovery/tests/agent_contract.rs`.

**Steps:** RED-test all one-shot command families available before shell creation for schema selection, human/JSON/NDJSON typed parity, clean stdout, structured stderr, and stable exit codes. GREEN: route those typed envelopes through shared renderers. Shell integration is explicitly deferred to Task 23C. Verify; commit `feat: integrate discovery agent output contracts`.

---

### Task 23A: Add the human shell over shared handlers

**Files:**
- Create: `amari-discovery/src/shell.rs`
- Modify: `amari-discovery/src/cli.rs`
- Create: `amari-discovery/tests/shell.rs`

**Steps:** RED-test `amari shell --project PATH`: inspect/recommend/plan use the session project by default, explicit command paths override it, nonexistent/changed paths produce typed errors, and help/capabilities/discovery/probe/exit retain the same typed results/authority as one-shot handlers. Implement only session project context, handler delegation, and input parsing. Verify; commit `feat: add amari discovery shell`.

---

### Task 23B: Add the provider-neutral AI validation contract

**Files:**
- Create: `amari-discovery/src/ai.rs`
- Create: `amari-discovery/tests/ai_contract.rs`
- Modify: `amari-discovery/src/lib.rs`
- Modify: `amari-discovery/src/capabilities.rs`
- Modify: `amari-discovery/tests/cli_capabilities.rs`

**Steps:**

1. RED: typed `GoalInterpreter` output validates against catalog IDs/limits; deterministic in-process echo adapter passes; malicious adapter returning uncatalogued IDs or execution requests fails.
2. GREEN: define trait and validation wrapper behind `ai`; update `Capabilities` so `contract_compiled` derives from `cfg!(feature = "ai")`, while `provider_configured` and `executable` remain false because v0.24 ships no concrete adapter; do not ship an external process/provider transport.
3. Run `cargo test -p amari-discovery --test ai_contract --test cli_capabilities --features ai` and the no-default capability test; commit `feat: add AI discovery adapter contract`.

---

### Task 23C: Add shell JSON/NDJSON contract parity

**Files:** Modify shell/render modules; create `amari-discovery/tests/shell_agent_contract.rs`.

**Steps:** RED-test one typed envelope per NDJSON line in shell machine mode, JSON request/response pairing, human/JSON/NDJSON semantic parity, stdout/stderr separation, session project context, and stable error/exit semantics. GREEN: route shell input/results through the Task 22 renderers without separate domain logic. Verify; commit `feat: expose shell agent output contracts`.

---

### Task 24A: Harden traversal boundaries

**Files:** Modify `amari-discovery/src/inspect/limits.rs`; create `amari-discovery/tests/inspection_paths.rs`.

**Steps:** RED-test nested symlink cycles, path traversal, ignored-directory escapes, depth/file/byte ceilings, and partial evidence. Implement only traversal fixes; verify; commit `test: harden discovery traversal`.

---

### Task 24B1: Harden malformed parser inputs

**Files:** Modify Cargo/npm/source inspectors and errors; create `amari-discovery/tests/inspection_malformed.rs`.

**Steps:** RED-test mixed encoding, malformed/malicious manifests, recursive workspace references, and huge token streams producing bounded warnings/errors. GREEN: implement parser guards only. Verify; commit `test: harden discovery parsers`.

---

### Task 24B2: Harden snapshot privacy and no-mutation guarantees

**Files:** Modify snapshot/evidence rendering; create `amari-discovery/tests/inspection_privacy.rs`.

**Steps:** RED-test secret-shaped content, absence of full-source/secrets in snapshots/errors, and unchanged target content/permissions/size/mtime. GREEN: implement redaction/evidence fixes only. Verify; commit `test: harden discovery input privacy`.

---

### Task 25A: Harden planner limits and domain outcomes

**Files:** Modify planner graph/normalization and protocol; create `amari-discovery/tests/planner_limits.rs`.

**Steps:** RED-test graph/rewrite ceilings, preserved partial evidence, and `NoApplicableCapability`/`InsufficientEvidence`/`Blocked` as successful typed outcomes. Implement only limit/outcome fixes; verify; commit `test: harden discovery planner limits`.

---

### Task 25B: Harden provenance and cross-process replay

**Files:** Modify planner plan/protocol; create `amari-discovery/tests/planner_replay.rs`.

**Steps:** RED-test stale project/catalog/input hashes, cross-process recommendation→plan replay, deterministic seeds, catalog identity, and no sensitive evidence leakage. Implement only provenance/replay fixes; verify; commit `test: harden discovery replay`.

---

### Task 26A: Fuzz worker framing and limit boundaries

**Files:** Modify worker/supervisor; create `amari-discovery/tests/probe_framing.rs`.

**Steps:** RED property/fuzz cases for random malformed frames, nested/oversized JSON, operation/node/input boundary combinations, unknown probes, output flooding, and repeated crash recovery. Implement only worker robustness fixes; verify; commit `test: harden probe framing`.

---

### Task 26B: Golden-test probe and error output contracts

**Files:** Modify render/error only as needed; create `amari-discovery/tests/probe_output.rs` and `amari-discovery/tests/golden/`.

**Steps:** RED golden tests for structured errors, exit codes, human/JSON/NDJSON parity, provenance fields, validated/refuted assumptions, and stdout/stderr separation. Implement only output fixes; run `cargo test -p amari-discovery --test probe_output --all-features`; commit `test: lock discovery output contracts`.

---

### Task 27: Write public and contributor documentation

**Files:**
- Modify: `amari-discovery/README.md`
- Create: `docs/guide/amari-discovery.md`
- Modify: `README.md`
- Modify: `CHANGELOG.md`
- Modify: `docs/README.md`
- Modify: `docs/roadmap/V0_20_0_TO_V0_25_0_RELEASE_SEQUENCE.md` only for verified status updates

**Steps:**

1. Add tested command examples for the human quick start and agent loop (`capabilities → inspect → recommend → plan → probe`).
2. Document JSON/NDJSON, exit codes, read-only/privacy limits, catalog generation, AI boundary, Rust/TS inspection, and why `amari-discovery` owns the `amari` command.
3. Run all doc tests and execute every shell example marked as runnable.
4. Commit `docs: document amari discovery workflows`.

---

### Task 28: Integrate crate publication order

**Files:**
- Modify: `.github/workflows/publish.yml`
- Modify: `scripts/verify-workflow-crates.sh` only if required
- Modify: `scripts/verify-publish-order.py` only if dependency changes require it

**Steps:**

1. Re-audit direct dependencies after implementation and extend the Task 1 order test if any were added.
2. Confirm `amari-discovery` remains after every direct dependency, including `amari-optimization` and `amari-rewrite`, and publish failures remain fatal.
3. Run `./scripts/verify-workflow-crates.sh`, `python3 scripts/verify-amari-binary-owner.py`, and `python3 scripts/verify-publish-order.py`.
4. Commit only if the final audit requires changes; otherwise record verification in the task log.

---

### Task 29: Validate feature-branch packaging and record aggregate release gates

**Files:**
- Create: `docs/releases/v0.24.0-aggregate-release-gates.md`
- Modify: package include/exclude metadata if archive inspection finds issues

**Steps:**

1. Document that the workspace version moves to 0.24.0 only after discovery/superpose and the separately decided rewrite expansion merge, the catalog is regenerated, and no stale internal 0.23 constraints remain.
2. Verify the local install contract against workspace paths, then inspect the unverified archive:

```bash
install_root=$(mktemp -d)
cargo install --path amari-discovery --root "$install_root"
test "$(find "$install_root/bin" -maxdepth 1 -type f -printf '%f\n')" = "amari"
"$install_root/bin/amari" capabilities --json | jq -e '.data.binary == "amari"'
"$install_root/bin/amari" discover search tropical --json | jq -e '.data.results | length > 0'
cargo package -p amari-discovery --allow-dirty --no-verify
tar -tf target/package/amari-discovery-*.crate
```

The local install proves the package source builds and installs one command. Do not report `--no-verify` as registry build verification: published `amari-holographic@0.23.0` lacks `superpose`.
3. Document full aggregate-release commands (`version-sync set/verify 0.24.0`, catalog regeneration, full `cargo package`, and publish dry-run) to execute after direct dependencies are available in release order.
4. Commit `docs: add v0.24 aggregate release gates`.

---

### Task 30: Add performance/token budgets and run final feature verification

**Files:**
- Create: `amari-discovery/tests/budgets.rs`
- Create: `amari-discovery/benchmarks.md`
- Create: `scripts/measure-discovery-binary.sh`
- Modify: implementation only for measured regressions

**Steps:**

1. RED: add portable functional-test bounds for `capabilities --json` bytes, compact search-result bytes, coarse small-fixture inspection duration, and deterministic recommendation output. Avoid fragile microsecond thresholds and do not inspect test-profile binary size.
2. Add a separate reporting script that runs `cargo build --release -p amari-discovery --bin amari`, measures `target/release/amari`, and records the result in `benchmarks.md`; report size rather than making it a machine-fragile integration assertion. Make minimal changes needed for functional budgets.
3. Run:

```bash
cargo fmt --all --check
git diff --check
./scripts/measure-discovery-binary.sh
cargo test -p amari-discovery --all-features
cargo test -p amari-discovery --no-default-features --test probe_engine --test cli_capabilities
cargo check -p amari-discovery --no-default-features
cargo test -p amari-holographic
cargo test --workspace --quiet
cargo clippy -p amari-discovery -p amari-holographic --all-targets --all-features -- -D warnings
./scripts/version-sync.sh verify 0.23.0
./scripts/verify-workflow-crates.sh
python3 scripts/verify-amari-binary-owner.py
RUSTDOCFLAGS="-D warnings" cargo doc -p amari-discovery -p amari-holographic --all-features --no-deps
```

4. Commit `test: verify amari discovery feature branch`.

---

### Task 31: Mandatory aggregate 0.24.0 release acceptance (post-merge)

**Context:** Execute on the aggregate release branch only after discovery/superpose and the separately decided rewrite expansion have merged.

**Steps:**

1. Rebase/merge all required 0.24 work; verify Task 10 is the sole canonical code implementation and PR #176 contributes documentation/handoff only.
2. Run `./scripts/version-sync.sh set 0.24.0`, audit all internal constraints/fixtures, and verify 0.24.0.
3. Regenerate Rust and authoritative wasm-pack/d.ts catalogs; require zero drift and catalog identity 0.24.0.
4. Run full workspace fmt/test/clippy/docs and all discovery feature combinations.
5. Run package/publish dry-runs for independent dependencies in actual order. Publish required 0.24 dependencies and wait for indexing; then run full `cargo package -p amari-discovery` **without** `--no-verify` and `cargo publish -p amari-discovery --dry-run`.
6. Extract the verified `.crate` archive to a temp directory, `cargo install --path` that extracted package, assert only `amari` is installed, and run capabilities/catalog smoke tests.
7. After publication, run `cargo install amari-discovery --version 0.24.0 --root <temp>` and repeat smoke tests. Record exact evidence in the release checklist.

This task is mandatory before claiming v0.24.0 release acceptance; feature-branch completion alone is not release completion.

---

## Feature-branch acceptance checklist

- [ ] `cargo install amari-discovery` installs exactly the `amari` binary.
- [ ] Root `amari` package no longer owns the placeholder binary.
- [ ] `amari capabilities` truthfully reports dynamic availability and schemas.
- [ ] Generated catalog covers every workspace package except the intentionally excluded `amari-discovery` tool and cannot drift silently.
- [ ] Semantic overlays validate against structural source records.
- [ ] Progressive search/detail/graph/example commands work in human and JSON modes.
- [ ] Rust and TypeScript project inspectors are bounded and read-only.
- [ ] Holographic candidate accumulation uses `superpose`, not attention-style `bundle`.
- [ ] Ranking exposes Pareto alternatives and full score/evidence breakdowns.
- [ ] Plans are normalized, bounded, hashed, and replay-compatible.
- [ ] Registered probes match direct Amari API results and obey limits.
- [ ] No arbitrary code/shell/network/project-write path exists.
- [ ] Human, JSON, and NDJSON outputs derive from the same typed responses.
- [ ] Structured errors and exit codes are stable and self-described.
- [ ] Optional AI cannot bypass catalog validation or probe authority.
- [ ] Workspace tests, focused clippy/docs, version sync, workflow coverage, and packaging checks pass.
- [ ] Publish order places `amari-discovery` after all of its 0.24.0 Amari dependencies.

## Aggregate v0.24.0 release acceptance checklist

- [ ] All required 0.24 branches are merged, including rewrite expansion and one canonical superpose implementation.
- [ ] Workspace and package metadata are synchronized to 0.24.0 with no stale internal constraints.
- [ ] Rust and generated WASM catalogs are regenerated at catalog identity 0.24.0.
- [ ] Full `cargo package` verification succeeds after direct 0.24 dependencies are available.
- [ ] `cargo publish --dry-run` succeeds in dependency order, including `amari-discovery`.
- [ ] Installation from the verified package archive installs only `amari` and passes smoke tests.
- [ ] Post-publish crates.io installation of `amari-discovery@0.24.0` passes the same smoke tests.
