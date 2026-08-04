# amari-discovery 0.24.x Schema Authority Design

> **Status:** Design for review. This is an additive 0.24.x contract layer; it
> preserves the shipped `amari.discovery/v1` protocol and all v0.24.0 probe
> behavior.

## 1. Goal

Give every bounded `amari-discovery` probe a machine-readable, versioned,
hash-identified wire contract that agents can inspect before invocation.

The current descriptors expose only opaque schema identifiers such as
`amari.discovery/probe/dual-polynomial-derivative/input/v1`. The actual input
shape is enforced by Rust DTOs at runtime, but agents cannot discover that
shape from the public JSON surface. This design closes that gap for all
executable 0.24.x probes without changing the existing protocol envelope.

## 2. Decisions

1. **Amari-first authority.** Implement the first complete authority layer in
   `amari-discovery`. Design it so the generic contract machinery can later be
   extracted into a reset Lonis without changing Amari's agent-facing shape.
2. **Lonis code is not reused.** The current Lonis repository is treated as
   obsolete/empty for this work. The Anima doctrine and Lonis direction remain
   authoritative, but no existing `lonis-schema` implementation is a dependency
   or compatibility source.
3. **Trait-first DTO contracts.** Rust request/response DTOs are the wire
   implementation authority. Each DTO declares a wire contract through a small
   trait, with derivation and semantic metadata attached at the DTO.
4. **Bounded derive now.** Add a focused `WireContract` derive that supports
   the Serde shapes used by current probe DTOs. Unsupported Serde shapes fail
   at compile time with explicit errors. Do not build a general schema macro
   framework in the first slice.
5. **Hybrid schema core.** Derive structural JSON Schema from the DTO, then
   attach authoritative semantic constraints, provenance, and compatibility
   metadata. JSON Schema expresses structure; domain validation remains in
   Rust.
6. **Keep protocol v1.** `amari.discovery/v1`, existing schema IDs, saved
   plans, probe IDs, output modes, and replay semantics remain valid. New
   fields and commands are additive only.
7. **All current probes in one slice.** The first implementation converts all
   13 executable probes, avoiding a mixed agent contract where only some
   schema IDs resolve.

## 3. Current seam

`amari-discovery` already has the needed foundations:

- stable protocol identity (`amari.discovery/v1`);
- typed envelopes with catalog, compatibility, replay, and input provenance;
- `amari schema ...` for curated protocol schema documents;
- strict request/response DTOs with `serde(deny_unknown_fields)` on probe
  inputs;
- declarative probe descriptors and a registry that checks compiled adapters
  against checked-in descriptors;
- process-isolated CLI execution and typed saved probe results.

The missing piece is the authority mapping from:

```text
ProbeDescriptor.input_schema / output_schema
```

to a resolvable schema document and canonical hash tied to the compiled DTO.

## 4. Architecture

### 4.1 Wire contract trait

Add an Amari-local trait, conceptually:

```rust
pub trait WireContract {
    fn schema_id(&self) -> &'static str;
    fn schema_role(&self) -> WireSchemaRole; // Input | Output
    fn structural_schema(&self) -> Value;
    fn semantic_constraints(&self) -> &'static [WireConstraint];
    fn examples(&self) -> &'static [WireExample];
    fn compatibility(&self) -> WireCompatibility;
}
```

Names may change during implementation, but the ownership rule does not: the
DTO declares the contract, and the registry collects it.

### 4.2 Bounded derive

Create `amari-discovery-macros` as a proc-macro crate for this workspace:

```rust
#[derive(WireContract)]
#[wire_contract(
    id = "amari.discovery/probe/dual-polynomial-derivative/input/v1",
    role = "input",
    compatibility = "additive_patch",
    constraints(finite_numbers, non_empty_coefficients)
)]
pub struct PolynomialDerivativeRequest { ... }
```

The derive must:

- use `schemars` to produce the structural schema;
- reject unsupported Serde/container shapes with a clear compile error;
- emit only deterministic, no-I/O code;
- never read the network, filesystem, environment, or current time;
- produce compile-time metadata usable by the registry.

`trybuild` tests cover accepted shapes and compile-fail cases.

### 4.3 Schema registry

Add an Amari-local registry that collects the request and response contract
for every compiled probe adapter. For each DTO it produces:

- existing stable schema ID;
- role (`input` or `output`);
- structural schema document;
- semantic constraint list;
- examples;
- compatibility class;
- canonical schema bytes;
- SHA-256 canonical schema hash.

The registry validates:

- schema ID syntax and role match;
- schema version equals the owning probe version;
- every executable probe has exactly one input and one output contract;
- every declarative descriptor schema ID has a compiled contract when the
  adapter is executable;
- duplicate IDs, role mismatches, and malformed hashes are catalog corruption.

### 4.4 Canonical document shape

The exported schema document is JSON Schema plus explicit Amari metadata, for
example:

```json
{
  "$id": "amari.discovery/probe/dual-polynomial-derivative/input/v1",
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "required": ["coefficients", "at"],
  "properties": {},
  "additionalProperties": false,
  "x-amari-schema-role": "input",
  "x-amari-protocol-version": "amari.discovery/v1",
  "x-amari-semantic-constraints": [],
  "x-amari-compatibility": "additive_patch"
}
```

Canonicalization uses the repository's deterministic JSON conventions. The
hash commits to the complete exported document, including metadata, so agents
can distinguish not only structural drift but semantic-contract drift.

### 4.5 Agent-facing commands

Keep existing commands unchanged and add schema resolution additively:

- `amari probe list` remains compact.
- `amari probe describe <id>` gains schema identities and hashes while
  preserving the existing descriptor fields.
- `amari probe schema <probe-id> --direction input|output` emits the complete
  document.
- `amari schema ...` remains the protocol-schema command; its catalog may gain
  additive references to probe-contract schema resolution, but its five
  existing protocol documents remain unchanged.

No full probe schema documents are embedded in `capabilities` or `probe list`.
Those commands return compact references/hashes to avoid context and output
bloat.

## 5. Compatibility and drift rules

Schema changes fall into three classes:

1. **No drift:** Rust DTO and exported document are byte/hash identical.
2. **Additive patch drift:** a new optional field or compatible annotation is
   added; the same `vN` schema ID may remain only when the contract hash is
   explicitly updated and release notes describe the additive change.
3. **Breaking drift:** a required field, type, unknown-field behavior, semantic
   constraint, or output meaning changes; the owning probe/schema version must
   advance (`vN` → `vN+1`).

Registry and catalog tests must fail when:

- a DTO-derived document changes without the checked-in hash changing;
- the schema ID changes without a version-compatible descriptor change;
- the descriptor and compiled adapter disagree;
- an executable probe lacks a complete input/output pair.

Saved plans and saved probe results continue to key replay to catalog/input
hashes and existing compatibility checks. Schema hashes are additive evidence,
not a replacement for v1 replay provenance.

## 6. 0.24.1 scope

Convert all current executable probes:

- CGT nim sum;
- core geometric product;
- dual polynomial derivative;
- holographic recall;
- holographic superposition;
- network shortest path;
- optimization Pareto front;
- rewrite normalize;
- rewrite infer rule;
- rewrite predecessors;
- surreal rational arithmetic;
- surcomplex rational division;
- tropical Viterbi.

The slice includes:

- `amari-discovery-macros` scaffold and bounded derive;
- trait-first wire-contract module;
- registry and canonical hashing;
- full schema documents for every probe DTO;
- CLI schema resolution and additive describe hashes;
- catalog/registry conformance tests;
- hostile nested DTO and malformed input tests;
- direct, process-isolated, and CLI parity tests for schema identity;
- deterministic catalog regeneration and catalog drift checks.

## 7. Explicit non-goals

- No change to `amari.discovery/v1`.
- No `lonis-schema` dependency.
- No general-purpose public schema macro beyond current probe DTO shapes.
- No schema inference for arbitrary internal mathematical Amari types.
- No attempt to encode all domain invariants in JSON Schema.
- No replacement of Rust runtime validation.
- No dynamic project code execution during schema generation.
- No 0.25 inverse-rewrite DTO expansion in this slice.

## 8. Extraction boundary for a reset Lonis

The following concepts should remain free of Amari-specific naming and logic
where practical:

- wire schema ID and role;
- structural schema document plus semantic constraints;
- canonical document bytes and hash;
- trait/derive contract collection;
- compact contract summary versus full document resolution;
- compatibility/drift classification.

Amari retains ownership of:

- probe IDs and capability IDs;
- mathematical DTOs and runtime validation;
- catalog/semantic provenance;
- plan/replay hashes;
- process isolation and resource limits;
- CLI presentation.

A later Lonis reset can absorb the generic contract traits, derive, registry,
hashing, and resolver traits. Amari then implements those traits for probe
contracts without changing the public Amari command surface.

## 9. Testing strategy

- Unit tests for ID/role/version/canonical-hash validation.
- `trybuild` compile-pass and compile-fail coverage for the derive.
- Golden schema tests for one representative DTO per Serde shape used.
- Round-trip and `deny_unknown_fields` conformance for every probe DTO.
- Registry tests proving descriptor/adapter/schema triples agree.
- CLI golden JSON tests for compact refs, full documents, and stable hashes.
- Property tests for deterministic canonicalization and hash stability.
- Existing workspace runtime matrix remains the acceptance gate.

## 10. Release framing

This is additive 0.24.x work. It may ship as `0.24.1` after:

- the complete normal warning-denied matrix;
- deterministic catalog evidence;
- package-shape evidence;
- publish-order verification;
- clean Rust and npm evidence if release artifacts are cut.

A schema-authority release still requires the normal Amari gitflow and Gates A
and B. Design or implementation PRs into `develop` do not themselves authorize
a version bump or release.
