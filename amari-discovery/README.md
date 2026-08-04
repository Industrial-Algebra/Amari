# amari-discovery

`amari-discovery` is the deterministic, offline-first discovery and planning
runtime for the Amari mathematical ecosystem. It is the sole workspace owner
of the installed `amari` command.

The command inspects Rust/Cargo and npm TypeScript projects, finds catalogued
Amari capabilities, produces replayable integration plans, and runs registered
bounded probes. It does not edit inspected projects or execute project code,
build scripts, lifecycle scripts, arbitrary commands, providers, or network
requests.

> **Release status:** the implementation is being prepared for Amari 0.24.0
> while the workspace remains at 0.23.0. Install from the workspace path for
> feature-branch validation; do not treat this preview as a published 0.24.0
> release.

Every `bash` block below is runnable with the stated checkout or project
context. Contributor-only catalog commands use `text` blocks because they have
additional toolchain prerequisites.

## Install from a checkout

```bash
cargo install --path amari-discovery
amari capabilities
```

Exactly one binary, `amari`, is installed. The root `amari` library package
does not own a second command.

## Human quick start

Run these commands from a Rust/Cargo or npm TypeScript project:

```bash
amari capabilities
amari discover search tropical
amari inspect .
amari recommend . --goal "differentiate a scalar polynomial with forward dual numbers"
amari probe list
```

Use `amari shell --project .` for an interactive session over the same typed
handlers. Run `amari <command> --help` for command-specific options.

## Agent loop

Machine consumers should start by negotiating capabilities and retain the full
versioned envelopes used for replay:

```bash
project=$(pwd)
artifacts=$(mktemp -d)

amari capabilities --json > "$artifacts/capabilities.json"
amari inspect "$project" --json > "$artifacts/inspection.json"
amari recommend "$project" \
  --goal "differentiate a scalar polynomial with forward dual numbers" \
  --json > "$artifacts/recommendation.json"

candidate=$(jq -r '.data.data.preferred.capability_id' "$artifacts/recommendation.json")
amari plan "$candidate" \
  --recommendation "$artifacts/recommendation.json" \
  --project "$project" \
  --json > "$artifacts/plan.json"

probe_id=$(jq -r '.data.steps[] | select(.kind == "probe") | .probe_id' "$artifacts/plan.json" | head -n1)
amari probe run "$probe_id" --plan "$artifacts/plan.json" --dry-run --json

printf '%s\n' '{"coefficients":[1.0,2.0,3.0],"at":2.0}' > "$artifacts/probe-input.json"
amari probe run amari-probe:dual:polynomial-derivative:v1 \
  --input "$artifacts/probe-input.json" \
  --json > "$artifacts/probe-result.json"
```

`plan` re-inspects the current project and validates the saved recommendation's
tool version, recall seed, catalog identity, project/input hashes, and saved
probe hashes. A dry run validates compatibility only; actual probe execution
requires an explicit typed input JSON file.

## Probe wire schema authority

Every executable probe resolves a complete DTO-derived input and output
schema. `probe describe` remains compact but now includes `schema_hashes` for
both directions:

```bash
amari probe describe amari-probe:dual:polynomial-derivative:v1 --json
```

Resolve the exported documents and their canonical SHA-256 hash explicitly:

```bash
amari probe schema amari-probe:dual:polynomial-derivative:v1 --direction input --json
amari probe schema amari-probe:dual:polynomial-derivative:v1 --direction output --json
```

The response remains an `amari.discovery/v1` envelope. Its `data.document` is
the exported JSON Schema plus Amari metadata, and `data.hash` is the canonical
SHA-256 hash of that document. For the dual polynomial-derivative input, the
complete document is:

```json
{
  "$id": "amari.discovery/probe/dual-polynomial-derivative/input/v1",
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "title": "PolynomialDerivativeRequest",
  "description": "Typed input for dual-number evaluation of a scalar polynomial and derivative.",
  "required": ["coefficients", "at"],
  "additionalProperties": false,
  "properties": {
    "coefficients": {
      "type": "array",
      "description": "Polynomial coefficients in descending-power order.",
      "items": { "type": "number", "format": "double" }
    },
    "at": {
      "type": "number",
      "format": "double",
      "description": "Point at which to evaluate the polynomial and its first derivative."
    }
  },
  "x-amari-schema-role": "input",
  "x-amari-protocol-version": "amari.discovery/v1",
  "x-amari-semantic-constraints": [
    {
      "id": "coefficient_count_limit",
      "description": "at most 5000 coefficients are accepted per request"
    },
    {
      "id": "finite_numbers",
      "description": "all coefficients and the evaluation point must be finite"
    },
    {
      "id": "nonempty_coefficients",
      "description": "the polynomial requires at least one coefficient"
    }
  ],
  "x-amari-examples": [
    {
      "label": "quadratic",
      "value": { "coefficients": [1.0, 2.0, 3.0], "at": 2.0 }
    }
  ],
  "x-amari-compatibility": "additive_patch"
}
```

Structural JSON Schema describes the accepted JSON shape. The
`x-amari-semantic-constraints` metadata is authoritative documentation for
checks that still require semantic Rust validation, such as finite numbers,
nonempty polynomials, exact rational bounds, term depth, and deterministic
truncation behavior. Agents should validate structure from the schema but must
not treat JSON Schema alone as proof that a payload satisfies the probe's
semantic contract.

The known but non-executable
`amari-probe:tropical:shortest-path:v1` descriptor remains declared-only: its
identity is visible, but the command does not fabricate a compiled contract
when no adapter is present.

## Output and errors

- Human output is the default.
- `--json` emits one `amari.discovery/v1` response envelope.
- `--ndjson` emits the same envelope as one newline-delimited record.
- `amari shell --json` accepts exactly one typed request.
- `amari shell --ndjson` accepts one bounded typed request per line and emits
  one response per line.
- Success is written to stdout. Structured errors are written to stderr.
- Stable error kinds and exit codes are reported by `amari capabilities --json`.

The curated request, response, goal, plan, and probe schemas are available
through `amari schema` and in [`schemas/`](schemas/).

## Inspection and privacy boundary

Inspection is read-only and bounded by file count, file size, aggregate bytes,
depth, and wall-clock limits. It follows neither external symlinks nor ignored
directory tunnels. Public evidence contains hashes and typed categories rather
than source text, secrets, absolute project roots, external targets, command
arguments, or raw parser/worker diagnostics.

Supported project evidence is intentionally narrow:

- Cargo manifests, lockfiles, Cargo platform configuration, and Rust source
  structure without running Cargo, rustc, build scripts, or project code.
- Root `package.json`, npm `package-lock.json` schema versions 2 and 3,
  TypeScript/JavaScript source structure, and generated Amari WASM `.d.ts`
  declarations without running npm, Node.js, lifecycle scripts, or project
  code. Yarn and pnpm locks are not interpreted in 0.24.0.

Usable bounded partial inspections are successful typed outcomes. Likewise,
`no_applicable_capability`, `insufficient_evidence`, and `blocked` are domain
outcomes rather than process failures.

## Probes and AI boundary

Only catalogued probe IDs with compiled adapters can run. Probe requests carry
only a probe ID, typed input, limits, and provenance. Public probe execution is
isolated in a restricted worker launched from the current `amari` executable;
callers cannot select a shell, executable, arguments, environment, working
directory, project handle, provider, or network transport.

The optional `ai` feature exposes a provider-neutral in-process
`GoalInterpreter` contract. Its validated wrapper bounds input/output, rejects
execution authority, and requires returned capability IDs to exist in the
embedded catalog. No concrete provider, subprocess transport, network client,
or additional execution authority ships with this crate.

## Catalog maintenance

Runtime discovery uses checked-in generated structure plus curated semantic
and probe metadata. `amari-discovery` is intentionally excluded from its own
structural catalog to prevent self-index drift.

From the workspace root, contributors regenerate the Rust catalog with:

```text
cargo run -p amari-discovery --example generate_catalog -- .
```

Regenerate the authoritative WASM surface with:

```text
./scripts/generate-discovery-wasm-surface.sh
```

Both generators use fixed output paths and atomic replacement. Review and
commit catalog changes; CI rejects drift, dangling semantic references, probe
manifest mismatches, or an unexpected discovery self-index.

## More documentation

See the [Amari Discovery Guide](../docs/guide/amari-discovery.md) for complete
human, agent, contributor, safety, protocol, and release-readiness workflows.
