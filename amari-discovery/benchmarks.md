# amari-discovery Budgets and Measurements

This file distinguishes portable functional budgets from machine-specific
release-binary reporting. Integration tests enforce agent-output and coarse
latency ceilings. Binary size is measured and reported, not asserted in tests,
because target triples, linkers, debug symbols, and toolchain revisions affect
it materially.

## Enforced functional budgets

`tests/budgets.rs` runs the real `amari` test binary and enforces:

| Surface | Ceiling | Approximate token ceiling | Rationale |
| --- | ---: | ---: | --- |
| `capabilities --json` | 8,192 bytes | 2,048 | Complete negotiation remains small enough for an agent preflight |
| `discover search tropical --json` | 4,096 bytes | 1,024 | Compact search does not expand into detail/graph payloads |
| deterministic recommendation | 32,768 bytes | 8,192 | Full preferred/alternative evidence remains bounded |
| minimal Rust inspection | 30 seconds | n/a | Coarse portable deadline, not a microbenchmark |

Token counts use a reporting approximation of four UTF-8 bytes per token; they
are not tokenizer-specific guarantees. Machine output must also remain one
compact newline-terminated record. Recommendation runs with identical project,
goal, catalog, and seed must be byte-identical.

A representative 2026-07-23 development-profile sample on x86_64 Linux
observed 5,083-byte capabilities output, 1,117-byte tropical search output,
4,673-byte deterministic recommendation output, and 2.57-second inspection of
the repository's bounded Rust fixture. These observations are context only;
the table above is the portable contract.

## Release binary measurement

Run from any directory in the repository:

```text
./scripts/measure-discovery-binary.sh
```

The script builds only `amari-discovery`'s `amari` binary in release mode,
measures `target/release/amari`, calculates its SHA-256, and replaces the
managed report below. It does not fail based on binary size and never measures
a test-profile executable.

<!-- discovery-binary-measurement:start -->

- Measured UTC: `2026-07-23`
- Build: `cargo build --release -p amari-discovery --bin amari`
- Toolchain: `rustc 1.98.0-nightly (91fe22da8 2026-06-21)`
- Host target: `x86_64-unknown-linux-gnu`
- Profile: `release`
- Binary: `target/release/amari`
- Size: `23982928` bytes (`22.87` MiB)
- SHA-256: `98969eb8c2651618fb3dd0a06a7c0b53135fe3bc37546537cb94a1c62d48bb3b`

<!-- discovery-binary-measurement:end -->

## Interpretation

The executable embeds `catalog/generated.json` and
`catalog/generated-wasm.json` so runtime discovery remains offline and cannot
drift from reviewed authority. The uncompressed Rust catalog is currently the
largest package input. Any future binary-size reduction must preserve catalog
identity, deterministic offline behavior, schema/probe contracts, and safety
tests; moving authority to an implicit network fetch is not an acceptable
optimization.
