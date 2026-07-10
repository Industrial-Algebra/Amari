# amari-discovery 0.24.0 Design

Date: 2026-07-09
Status: validated design for implementation planning
Package: `amari-discovery`
Installed binary: `amari`

## 1. Purpose

`amari-discovery` is the agentic front door to Amari as a mathematical operating system.

Its primary purpose is to help coding agents inspect a project, discover which Amari capabilities apply, understand the mathematical mapping, compare alternatives, produce a reproducible integration plan, and run bounded read-only probes before implementation. It also provides a polished human interface over the same command engine.

This is not merely an API index and not a general coding agent. It combines:

- Karpal's progressive, token-efficient source/API discovery;
- Schubert's discover/recommend/explore interaction model;
- Proserpina's dynamic capabilities, dry-run planning, structured errors, and human/agent output parity;
- Amari's own mathematical domains as the recommendation substrate.

The v0.24.0 implementation is Amari-only. A versioned protocol should permit future Schubert/Karpal federation without requiring external-project support now.

## 2. Core decisions

- The product has three layers: **Discover**, **Plan**, and **Experiment**.
- Agentic project discovery is primary; the human interface is a first-class projection of the same core.
- The deterministic, offline core is authoritative.
- An optional provider-neutral AI adapter may translate prose into typed goals and summarize results, but cannot bypass validation or execution limits.
- The catalog is a generated structural index plus curated semantic overlays.
- The planner dogfoods Amari: holographic recall, network relationships, tropical/multi-objective ranking, rewriting, and contracts/probes.
- Target projects are read-only. The tool may inspect, plan, emit artifacts, and run registered probes; it never edits project files.
- Rust/Cargo and JavaScript/TypeScript (`amari-wasm`) consumers are both first-class in v0.24.0.
- The crates.io package is `amari-discovery`; the installed command is `amari`.
- The current root-package `amari` placeholder binary is retired so there is one authoritative command.

## 3. Product model

### 3.1 Discover

Progressively reveal:

- Amari crates and feature gates;
- mathematical concepts and problem shapes;
- public types, traits, functions, macros, and examples;
- capability dependencies and composition paths;
- maturity, limitations, runtime characteristics, and available probes;
- how a target project already uses Amari and where relevant capabilities are missing.

Queries may begin with an API symbol (`RationalSurreal`), a concept (`shortest paths under uncertainty`), or a project path.

### 3.2 Plan

Map a typed project snapshot and goal into ranked mathematical integration plans. Each plan must explain:

- why the capability applies;
- which alternatives were considered;
- exact crates, feature gates, symbols, and examples;
- prerequisites and invalidating conditions;
- expected implementation and runtime costs;
- assumptions and missing evidence;
- specific probes that can increase or decrease confidence;
- suggested tests and verification criteria.

Planning is read-only and deterministic. It is the equivalent of Proserpina's dry-run, generalized to mathematical integration.

### 3.3 Experiment

Run bounded, typed, registered Amari operations against structured inputs. Probes validate a recommendation's assumptions or demonstrate a capability with real library code.

A probe is not arbitrary Rust execution. It declares:

- a versioned input/output schema;
- required compile-time features;
- cost class and resource ceilings;
- deterministic seed behavior;
- timeout and iteration/node limits;
- side effects (normally none);
- provenance and replay requirements.

## 4. Command surface

```text
amari capabilities

amari discover search <query>
amari discover detail <capability-or-symbol>
amari discover graph <capability-or-symbol>
amari discover example <capability-or-symbol>

amari inspect [PATH]
amari recommend [PATH] --goal <text>
amari recommend [PATH] --goal-file <goal.json>
amari plan <candidate-id> --project PATH

amari probe list
amari probe describe <probe-id>
amari probe run <probe-id> --input <input.json>
amari probe run <probe-id> --plan <plan.json> --dry-run

amari shell [--project PATH]
amari schema [request|response|goal|plan|probe]
```

All commands have concise human output by default and `--json` machine output. Commands that emit many independent records may also support `--ndjson`.

`amari shell` is an interactive client over the same typed engine. It gains no hidden commands, mutation authority, or separate semantics.

## 5. Package architecture

`amari-discovery` is a workspace package with both a library and binary target:

```text
amari-discovery/
├── Cargo.toml
├── catalog/
│   ├── generated.json
│   └── semantic/
├── src/
│   ├── lib.rs
│   ├── protocol.rs
│   ├── catalog/
│   ├── inspect/
│   │   ├── cargo.rs
│   │   └── npm.rs
│   ├── planner/
│   │   ├── recall.rs
│   │   ├── graph.rs
│   │   ├── rank.rs
│   │   └── normalize.rs
│   ├── probes/
│   ├── ai.rs
│   ├── render.rs
│   ├── error.rs
│   └── main.rs
└── tests/
```

The discovery package depends directly on individual Amari domain crates rather than on the umbrella `amari` package. It must not become an umbrella feature, avoiding a package dependency cycle.

The root package's placeholder `src/main.rs` is removed or automatic binary discovery is disabled. `amari-discovery` owns `[[bin]] name = "amari"`.

## 6. Versioned protocol

The library exposes typed, serializable request/response structures under a schema identifier such as `amari.discovery/v1`.

Core types:

- `Capabilities`
- `ProjectSnapshot`
- `GoalSpec`
- `CapabilityRecord`
- `Evidence`
- `Recommendation`
- `CandidatePlan`
- `PlanStep`
- `ProbeDescriptor`
- `ProbeRequest`
- `ProbeResult`
- `DiscoveryError`

Stable capability IDs must not depend on display names. Suggested shape:

```text
amari:<crate>:<module>:<capability>
```

Every machine response includes:

- `schema_version`;
- tool and catalog versions;
- project/catalog/input hashes where relevant;
- deterministic seed where relevant;
- warnings and evidence references;
- compatibility and replay metadata.

Human and machine renderers consume the same typed response objects.

## 7. Dynamic capabilities

`amari capabilities` reports what the installed binary can do in the current environment, not merely what exists in Amari.

It includes:

- tool, protocol, and embedded catalog versions;
- supported project inspectors;
- output formats;
- compiled probe adapters;
- host and target information;
- optional AI-adapter availability;
- default resource ceilings;
- known feature gates;
- stable exit-code map.

A capability may be:

- **known** — represented in the catalog;
- **available** — compatible with the inspected project/environment;
- **executable** — has a compiled probe adapter in this binary.

These states must never be conflated.

## 8. Hybrid catalog

### 8.1 Generated structural index

CI generates structural metadata from Amari source and manifests:

- workspace crates and descriptions;
- dependency and feature graph;
- public modules, items, signatures, and trait relationships;
- examples and documentation links;
- target/feature restrictions;
- probe registrations.

Generated data is checked into the discovery package so a crates.io installation does not need an Amari source checkout.

### 8.2 Curated semantic overlays

Semantic overlays add information source syntax cannot provide:

- mathematical concept names and aliases;
- problem shapes and appropriate use cases;
- expected input/output domains;
- assumptions and failure conditions;
- composition relationships;
- alternatives and trade-offs;
- maturity and stability tier;
- implementation/runtime cost hints;
- recommended examples, probes, and tests.

CI rejects overlays referencing missing crates, features, symbols, examples, or probes. Catalog generation must be deterministic and produce a content hash.

### 8.3 Local refresh

A local workspace mode may inspect newer source and report drift from the embedded catalog. Public operation never requires this mode.

## 9. Project inspection

Inspection is read-only and bounded. Source contents are not persisted by default.

### 9.1 Rust/Cargo

Collect:

- Cargo metadata and resolved dependencies;
- enabled and available Amari features;
- imported/used Amari symbols;
- source module and domain vocabulary;
- examples, tests, benchmarks, and target configuration;
- README/design comments that express project intent;
- current Amari versions and API surface;
- platform constraints such as WASM, `no_std`, GPU, or native dependencies.

### 9.2 JavaScript/TypeScript

Collect:

- package manifests and lockfile-resolved versions;
- `@justinelliottcobb/amari-wasm` presence and version;
- JS/TS imports and generated `.d.ts` availability;
- build target and bundler context;
- tests/examples and project vocabulary;
- mapping from WASM exports to shared capability IDs.

### 9.3 `ProjectSnapshot`

The snapshot records extracted signals, source locations, hashes, and confidence—not an opaque prose summary. Agents may save and pass snapshots between commands.

## 10. Amari-native planner

The planner uses Amari where it provides a genuine computational advantage. Each stage remains inspectable and has a deterministic fallback.

### 10.1 Holographic candidate recall

Deterministic holographic encodings map project/goal concepts into a semantic candidate space. Recall expands beyond literal symbol matches and surfaces related capabilities.

The v0.24 `BindingAlgebra::superpose` addition is a prerequisite for correct accumulation semantics. Holographic similarity only generates candidates; it never establishes correctness or final ranking.

### 10.2 Capability graph expansion

Capabilities, concepts, requirements, alternatives, examples, and probes form a typed graph. Relationships include:

- `implements`
- `requires`
- `composes_with`
- `alternative_to`
- `accelerates`
- `verifies`
- `invalid_when`
- `demonstrated_by`
- `probed_by`

`amari-network` operations identify composition paths and related capabilities.

### 10.3 Tropical and multi-objective ranking

Candidate paths are ranked across multiple dimensions:

- applicability;
- evidence quality;
- integration effort;
- maturity/stability;
- runtime cost;
- platform compatibility;
- verification strength;
- uncertainty and risk.

Tropical path costs model integration sequences. Multi-objective optimization preserves a Pareto set rather than collapsing every trade-off into a single opaque score. A preferred candidate uses deterministic, documented tie-breaking.

### 10.4 Rewrite normalization

`amari-rewrite` canonicalizes plan steps, removes redundant dependency/feature operations, and applies known safe transformations. Normalization is bounded and emits a trace.

### 10.5 Contracts and evidence

Preconditions are checked against project evidence and probe results. Unsatisfied requirements mark a plan blocked or lower confidence. They are never silently ignored.

Each recommendation contains:

- preferred plan;
- Pareto alternatives;
- score breakdown;
- evidence and source locations;
- assumptions and missing information;
- blocking conditions;
- suggested probes and tests.

## 11. Probe registry

The standard v0.24 binary should ship representative CPU-safe probes for:

- core geometric algebra;
- tropical and dual computation;
- network and optimization;
- holographic storage/retrieval and additive superposition;
- CGT, surreal, and surcomplex arithmetic;
- rewrite normalization, inverse search, and inference.

Catalog records may describe other Amari domains even when no compiled probe exists. `amari capabilities` and `amari probe list` expose the distinction.

GPU and Borsalino probes are deferred to 0.25.0.

Probe output includes:

- typed result;
- selected backend;
- duration/resource observations;
- determinism and seed;
- warnings;
- project/catalog/input hashes;
- assumptions validated or refuted.

## 12. Optional AI adapter

The deterministic core accepts a typed `GoalSpec`. The optional AI adapter may:

- translate natural-language goals into `GoalSpec`;
- ask for missing information in human shell mode;
- summarize evidence-backed recommendations.

It may not:

- introduce uncatalogued capabilities;
- alter deterministic score components;
- execute probes directly;
- bypass resource limits;
- edit target projects;
- hide uncertainty or missing evidence.

The library defines a provider-neutral adapter trait. A reversible first implementation may use a structured stdin/stdout external-command adapter, allowing any agent harness or local model to participate without embedding provider credentials in the core.

All AI use is explicit in output provenance. The tool remains fully useful offline without it.

## 13. Output and error contract

- Human-readable output is the default.
- `--json` emits one typed response on stdout.
- `--ndjson` emits independently parseable events.
- Progress and diagnostics go to stderr.
- Structured machine errors go to stderr when JSON/NDJSON mode is active.
- Exit-code meanings are stable and self-described by `amari capabilities`.

Domain outcomes such as `no_applicable_capability`, `insufficient_evidence`, or `plan_blocked` are represented explicitly. They are not internal errors.

Errors provide actionable remediation in human mode and stable `kind`, `message`, and `details` fields in machine mode.

## 14. Safety and authority

`amari-discovery` has no target-project mutation authority in v0.24.0.

It must not:

- modify project files;
- install packages;
- invoke arbitrary shell commands;
- evaluate arbitrary Rust/JavaScript;
- make implicit network calls;
- execute unregistered probes;
- follow unbounded graph/rewrite/synthesis searches.

Project traversal must honor explicit path, file-size, node-count, time, and symlink boundaries. Probes fail with partial evidence when limits are reached.

## 15. Verification strategy

### 15.1 Catalog integrity

- deterministic generation snapshots;
- every semantic reference validated against source;
- feature/dependency graph consistency;
- stale symbol/example/probe detection;
- catalog hash reproducibility.

### 15.2 Recommendation quality

- fixtures derived from realistic IA-style Rust and TS projects;
- expected candidate and alternative sets;
- negative/no-match cases;
- property tests for score ordering and Pareto dominance;
- metamorphic tests:
  - irrelevant files do not change recommendations;
  - satisfying a prerequisite improves/unblocks the same plan;
  - removing evidence never increases confidence;
  - deterministic seeds reproduce ranking.

### 15.3 Probe correctness

- direct library API versus CLI probe parity;
- input schema rejection;
- timeout/resource-limit behavior;
- replay compatibility and drift detection;
- no-write/no-network assertions;
- headless-safe behavior.

### 15.4 Protocol and UX

- golden JSON/NDJSON schemas;
- human and machine outputs derive from identical data;
- stable exit-code mapping;
- stdout/stderr separation;
- progressive-discovery token budgets;
- latency budgets for catalog search and project inspection;
- offline echo AI-adapter tests.

### 15.5 Packaging

- `cargo install amari-discovery` installs exactly the `amari` command;
- the umbrella package no longer publishes the placeholder binary;
- crates.io package includes the generated catalog and semantic overlays;
- minimal and standard-probe feature combinations compile;
- Rust and JS/TS fixture workflows run in CI.

## 16. v0.24.0 scope

Included:

- `amari-discovery` package and `amari` command;
- versioned human/agent protocol;
- dynamic capabilities and schema discovery;
- hybrid catalog across all Amari workspace crates;
- Rust/Cargo and JS/TS project inspection;
- Amari-native candidate recall, graph expansion, ranking, and plan normalization;
- read-only recommendations and replayable plans;
- representative CPU-safe probe adapters;
- optional provider-neutral AI-adapter contract;
- full provenance, limits, and structured errors.

Excluded:

- project modification or patch application;
- arbitrary code/shell execution;
- external-project federation;
- GPU modernization, Borsalino integration, or new GPU probes;
- a new Amari programming-language parser or general runtime syntax;
- hidden network/provider dependencies.

## 17. Future direction

0.25.0 may add GPU/Borsalino capability and probe adapters after the `wgpu` modernization.

Later releases may federate Schubert, Karpal, and other IA discovery providers through the same protocol; add richer provider integrations; or expose the discovery engine through MCP or other transports. The CLI remains the universal baseline.

## 18. Success criteria

v0.24.0 succeeds when:

1. An agent can self-discover the installed tool and schemas without reading external docs.
2. Given an Amari-consuming Rust or TS project, it can identify applicable capabilities with evidence and alternatives.
3. It can produce a deterministic, replayable integration plan without modifying the project.
4. It can run registered probes that validate key assumptions using real Amari operations.
5. Humans can perform the same workflow with readable commands and guided interaction.
6. Recommendations expose provenance, confidence, costs, and missing evidence rather than opaque scores.
7. The catalog cannot silently drift from the Amari source surface.
8. The tool works offline without an AI provider.
