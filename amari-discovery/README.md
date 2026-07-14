# amari-discovery

`amari-discovery` is the agent-first discovery and planning runtime for the
Amari mathematical ecosystem. It installs the `amari` command.

The command is designed to inspect Rust and JavaScript/TypeScript projects,
find relevant Amari capabilities, produce replayable integration plans, and
run registered bounded probes. It does not edit inspected projects or execute
arbitrary project code, shell commands, or network providers.

This crate is under active development for Amari 0.24.0 while the feature
branch remains at workspace version 0.23.0. Deterministic catalog discovery is
implemented; project inspection, recommendations, planning, and bounded probe
execution remain in development.
