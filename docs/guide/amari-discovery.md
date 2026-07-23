# Amari Discovery Guide

`amari-discovery` installs the `amari` command for humans and software agents
that need to understand and integrate the Amari ecosystem. It combines a
checked-in capability catalog, bounded project inspection, deterministic
candidate retrieval and ranking, replayable plans, and registered mathematical
probes.

This guide describes the 0.24.0 feature surface while the workspace is still at
0.23.0. Passing feature-branch checks is not a 0.24.0 release claim; aggregate
versioning, catalog regeneration, package verification, publication, and
post-publication installation remain separate release gates. Every `bash`
block in this guide is runnable in the stated checkout or inspected-project
context. Contributor commands with additional WASM/toolchain prerequisites use
`text` blocks instead.

## Why `amari-discovery` owns `amari`

The root `amari` crate is the Rust library umbrella. A second placeholder
binary there would make `cargo install` ownership ambiguous and could drift
from the discovery protocol. The workspace therefore has one binary owner:
`amari-discovery` declares `[[bin]] name = "amari"`, while the root package is
library-only. CI verifies this invariant.

## Installation during 0.24.0 development

From an Amari checkout:

```bash
cargo install --path amari-discovery
amari --version
amari capabilities
```

The path install must create exactly one file named `amari` in the install
root's `bin` directory. Registry installation is not accepted until the
0.24.0 dependency chain has been published and indexed.

## Human workflow

### 1. Negotiate the installed surface

```bash
amari capabilities
```

Capabilities report the binary name, protocol version, output modes, catalog
identity, inspectors, feature-gated probe availability, schemas, AI contract
state, and stable exit codes. Consumers should not infer availability from a
catalog entry alone: a descriptor can be known while its implementation is not
compiled into the current binary.

### 2. Explore the catalog

```bash
amari discover search tropical
amari discover detail amari:amari-tropical:sequence:viterbi
amari discover graph amari:amari-tropical:sequence:viterbi
amari discover example amari:amari-dual:autodiff:forward-derivative
```

`search` returns compact ranked summaries. `detail`, `graph`, and `example`
progressively expose richer catalog evidence without inspecting a project.

### 3. Inspect without executing the project

```bash
amari inspect .
```

Inspection recognizes Rust/Cargo projects, npm TypeScript projects, and mixed
roots. It reads a bounded deterministic subset of regular files and returns
complete or typed partial evidence. It does not invoke Cargo, rustc, npm,
Node.js, lifecycle scripts, build scripts, project binaries, or the network.

### 4. Recommend and plan

```bash
amari recommend . --goal "differentiate a scalar polynomial with forward dual numbers"
```

Recommendations include a preferred Pareto candidate, alternatives, the full
minimization score/evidence breakdown, prerequisites, compatibility, planned
steps, and relevant probes. `no_applicable_capability`,
`insufficient_evidence`, and `blocked` are successful domain outcomes.

A replayable plan requires the saved machine recommendation, candidate ID, and
current project. See the agent workflow below.

### 5. Inspect registered probes

```bash
amari probe list
amari probe describe amari-probe:dual:polynomial-derivative:v1
probe_input=$(mktemp)
printf '%s\n' '{"coefficients":[1.0,2.0,3.0],"at":2.0}' > "$probe_input"
amari probe run amari-probe:dual:polynomial-derivative:v1 --input "$probe_input"
```

Only registered, compiled adapters run. The public CLI routes execution through
a restricted process worker and reports process isolation only after validating
the worker frame, result, and provenance.

## Agent workflow: capabilities → inspect → recommend → plan → probe

The following workflow requires `jq` and should be run from the project being
analyzed:

```bash
project=$(pwd)
artifacts=$(mktemp -d)

amari capabilities --json > "$artifacts/capabilities.json"
jq -e '.schema_version == "amari.discovery/v1" and .data.binary == "amari"' "$artifacts/capabilities.json"

amari inspect "$project" --json > "$artifacts/inspection.json"
jq -e '.provenance.project_hash and (.provenance.replay.required_hashes | length > 0)' "$artifacts/inspection.json"

amari recommend "$project" \
  --goal "differentiate a scalar polynomial with forward dual numbers" \
  --json > "$artifacts/recommendation.json"
jq -e '.data.status == "recommended"' "$artifacts/recommendation.json"

candidate=$(jq -r '.data.data.preferred.capability_id' "$artifacts/recommendation.json")
amari plan "$candidate" \
  --recommendation "$artifacts/recommendation.json" \
  --project "$project" \
  --json > "$artifacts/plan.json"
jq -e '.data.plan_hash and (.data.steps | length > 0)' "$artifacts/plan.json"

probe_id=$(jq -r '.data.steps[] | select(.kind == "probe") | .probe_id' "$artifacts/plan.json" | head -n1)
amari probe run "$probe_id" --plan "$artifacts/plan.json" --dry-run --json > "$artifacts/probe-dry-run.json"
jq -e '.data.executable == true and .data.compatible == true' "$artifacts/probe-dry-run.json"

printf '%s\n' '{"coefficients":[1.0,2.0,3.0],"at":2.0}' > "$artifacts/probe-input.json"
amari probe run amari-probe:dual:polynomial-derivative:v1 \
  --input "$artifacts/probe-input.json" \
  --json > "$artifacts/probe-result.json"
jq -e '.data.isolation == "process" and .data.result.output.derivative == 6.0' "$artifacts/probe-result.json"
```

The saved recommendation is not trusted merely because its internal hashes are
self-consistent. `plan` re-inspects the project and re-derives the current tool
version, deterministic recall seed, catalog identity, project/input hashes,
probe hashes, compatibility, warnings, provenance, normalized steps, and plan
hash. Malformed SHA-256 values, stale authority, or changed projects fail with
a typed replay error.

A probe `--plan --dry-run` is compatibility-only and never executes the probe.
Actual execution requires an explicit typed JSON file via `--input`; plans
cannot smuggle a project path, executable, command arguments, shell configuration, provider, or
network authority into a worker request.

## JSON, NDJSON, and shell contracts

All modes project the same typed response envelopes:

- default: human-readable output;
- `--json`: exactly one JSON envelope and trailing newline;
- `--ndjson`: exactly one JSON envelope as one NDJSON record;
- `amari shell --json`: exactly one bounded request from stdin and one response;
- `amari shell --ndjson`: one bounded request and response per non-empty line.

A machine shell request has this shape:

```json
{"schema_version":"amari.discovery/v1","command":"discover.search","arguments":{"query":"tropical"}}
```

For a streaming session:

```bash
printf '%s\n' \
  '{"schema_version":"amari.discovery/v1","command":"capabilities","arguments":{}}' \
  '{"schema_version":"amari.discovery/v1","command":"discover.search","arguments":{"query":"tropical"}}' \
  '{"schema_version":"amari.discovery/v1","command":"probe.list","arguments":{}}' \
  | amari shell --ndjson
```

Emit curated schemas with `amari schema request --json`, replacing `request`
with `response`, `goal`, `plan`, or `probe`. Checked-in schemas live in
`amari-discovery/schemas/` and use JSON Schema 2020-12.

### Stable errors and streams

Successful output goes to stdout. Human and structured errors go to stderr;
machine errors are single JSON/NDJSON records with `details.exit_code`. The
running binary advertises the authoritative map. The current v1 classes are:

| Error kind | Exit code |
| --- | ---: |
| `invalid_id`, `invalid_input` | 2 |
| `catalog_corruption` | 3 |
| `inspection_failure` | 4 |
| `probe_unavailable` | 5 |
| `probe_failed` | 6 |
| `limit_exceeded` | 7 |
| `io` | 8 |
| `serialization` | 9 |
| `not_implemented` | 69 |
| `internal` | 70 |

Scripts should negotiate this map instead of hard-coding future additions.
Domain non-recommendations are successful values and do not use error exits.

## Read-only and privacy model

Inspection and replay never modify the target project. Traversal is bounded by
considered regular files, per-file bytes, aggregate bytes, depth, and elapsed
wall time. It does not follow symlinks outside the canonical root, symlink
cycles, ignored-directory tunnels, or recursively declared nested workspaces.

Public evidence intentionally excludes:

- complete source text and secret-shaped content;
- absolute project roots and external symlink targets;
- raw malformed workspace paths, lockfile source URLs, or unsafe package names;
- Cargo runner/linker directories and command arguments;
- raw parser diagnostics and worker stderr;
- untrusted saved warnings or compatibility reasons.

Source locations are content-addressed. Malformed evidence is represented by
fixed categories, and usable bounded partial snapshots remain typed successful
outcomes.

## Rust and TypeScript inspection scope

### Rust/Cargo

The inspector reads Cargo manifests and lockfiles, workspace membership, target
and feature declarations, `.cargo/config.toml` platform evidence, Rust import
and API usage structure, attributes, cfgs, tests, examples, benches, and bounded
vocabulary. It does not run dependency resolution or compile the project.

### npm TypeScript

The npm inspector supports only root `package.json` and root
`package-lock.json` schema versions 2 and 3 in 0.24.0. It reads bounded
TypeScript/JavaScript structure and the installed Amari WASM package's generated
`.d.ts` declarations. Missing or malformed optional lockfiles are typed and
nonfatal. Yarn and pnpm locks are excluded, and no package scripts execute.

Rust and TypeScript evidence map into shared semantic capability IDs. WASM API
authority comes from generated `.d.ts`, not hand-maintained TypeScript claims.

## Probe isolation and limits

Probe descriptors define schemas, feature requirements, cost, determinism,
side effects, network policy, timeout, input/output bytes, and work ceilings.
Callers may tighten but never loosen descriptor limits. In-process library
execution truthfully reports `cooperative`; the CLI reports `process` only
following successful supervisor validation.

The worker protocol is exactly one non-empty, bounded, four-byte big-endian
length-prefixed JSON request and response frame. The supervisor launches only
its current executable with the sole hidden `__probe-worker` argument, clears
the inherited environment, uses a neutral working directory, drains stdout and
stderr concurrently under independent caps, and kills and reaps on deadline or
cap violation. Raw diagnostics never become public errors.

## Optional AI contract

Feature `ai` exposes the provider-neutral `GoalInterpreter` trait and
`ValidatedGoalInterpreter`. Validation bounds input and output, rejects empty
or duplicate fields, verifies capability IDs against the embedded catalog, and
rejects any requested execution authority.

It does **not** ship a provider, subprocess protocol, shell bridge, network
client, credential path, or probe bypass. Applications embedding the trait
remain responsible for obtaining an interpretation; Amari only validates its
bounded typed result.

## Contributor catalog workflow

The runtime catalog is hybrid:

1. `catalog/generated.json` records deterministic Rust workspace structure;
2. `catalog/generated-wasm.json` records the authoritative generated `.d.ts`
   surface;
3. `catalog/semantic/*.toml` supplies curated concepts and relations;
4. `catalog/probes.toml` declares bounded probe authority.

`amari-discovery` is excluded from generated structural records so the tool does
not self-index and drift whenever its own implementation changes.

Regenerate Rust structure from the workspace root:

```text
cargo run -p amari-discovery --example generate_catalog -- .
```

Regenerate the WASM surface with the pinned contributor path:

```text
./scripts/generate-discovery-wasm-surface.sh
```

Generators accept no caller-selected output path, write only fixed catalog
files atomically, and perform no runtime network access. After regeneration,
run:

```text
cargo test -p amari-discovery --test catalog_generation --all-features
cargo test -p amari-discovery --test catalog_integrity --all-features
cargo test -p amari-discovery --test catalog_wasm --all-features
```

CI also checks schema goldens, catalog identity, semantic references, probe
registry parity, the sole binary owner, exhaustive discovery-test sharding,
and dependency-safe publication order.

## Release boundary

Feature-branch completion proves source and path-install behavior only. It does
not prove a registry package can resolve unpublished 0.24.0 dependencies.
Aggregate release acceptance occurs after discovery, additive holographic
superposition, and the separately approved rewrite expansion are merged. The
release branch must then move every workspace package and internal constraint
to 0.24.0, regenerate both catalogs, run full package/publish dry-runs in
actual dependency order, inspect and install the verified archive, publish,
and repeat installation from the registry.
