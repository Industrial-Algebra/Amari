# Amari 0.20.0 → 0.23.0 Release Sequence

Date: 2026-04-30
Current planning baseline: `0.20.0` release candidate work is complete pending merge/release.

## Release posture

The next releases should preserve the separation between stabilization, algebraic expansion, new crate introductions, and GPU follow-up work.

The key planning change is that the `amari-gpu` follow-up issues raised after the 0.20.0 hardening pass are no longer planned as a `0.20.1` fast-follow. They are better handled in the `0.23.0` cycle, where `amari-cli`, `amari-surcomplex`, and new crate coverage create a more natural context for GPU tooling, calibration, and backend migration work.

Patch releases between these milestones should remain bug-fix only.

## 0.20.0 — `amari-gpu` stabilization baseline

Theme: correctness-first GPU stabilization.

Primary outcome:

- land the `amari-gpu` hardening PR as the known-good GPU baseline
- restore/narrow public GPU APIs where appropriate
- validate public GPU surfaces against CPU baselines
- document GB10 and RTX 5080 hardware validation
- document benchmark/crossover posture honestly
- distinguish GPU-backed, GPU-recommended, CPU-preferred, fallback, and infrastructure paths

Non-goals:

- do not require every GPU-backed path to beat CPU
- do not include the `wgpu 0.19 -> 29` migration
- do not add new broad GPU surfaces without CPU-baseline tests

Patch lane after 0.20.0:

- reserve `0.20.x` for packaging fixes, serious correctness bugs, or documentation corrections
- do not treat benchmark refinement or backend migration as mandatory `0.20.1` work

## 0.21.0 — `amari-tropical` and `amari-dual` extension release

Theme: additive algebraic expansion.

Primary outcome:

- considerably extend `amari-tropical`
- considerably extend `amari-dual`
- preserve existing crate identities and downstream compatibility
- focus on semiring abstractions, compiler/scheduling use cases, higher-order/batched AD, and practical examples

Secondary / optional:

- update `amari-fusion` examples only where needed to consume the new tropical/dual capabilities
- defer GPU integration of the new APIs unless it is small, obvious, and already covered by CPU-baseline tests

Non-goals:

- do not reopen the broad `amari-gpu` redesign during 0.21.0
- do not fold the `wgpu 29` migration into 0.21.0 unless it becomes unavoidable for compatibility

## 0.22.0 — `amari-cgt` and `amari-surreal`

Theme: combinatorial game theory and surreal-number foundations.

Primary outcome:

- introduce `amari-cgt` for combinatorial game theory
- introduce `amari-surreal` for surreal numbers
- define their public APIs, tests, documentation, and examples
- establish integration points with existing algebraic crates where appropriate

Non-goals:

- do not require immediate GPU acceleration for `amari-cgt` or `amari-surreal`
- do not block these crates on `amari-gpu` follow-up work

## 0.23.0 — `amari-surcomplex`, `amari-cli`, and `amari-gpu` follow-up

Theme: new front-door tooling plus GPU revisit.

Primary outcome:

- introduce `amari-surcomplex`
- introduce `amari-cli`
- revisit `amari-gpu` in light of the new 0.21.0 and 0.22.0 crates
- decide which new operations are practical GPU candidates
- move benchmark/crossover and dispatch refinement work out of the 0.20.x patch lane

GPU follow-up issues planned for this cycle:

- #137 — Add missing CPU baseline timings to `amari-gpu` benchmark harnesses
- #138 — Add release-mode or Criterion benchmarks for `amari-gpu`
- #139 — Implement hardware-aware calibrated dispatch for `amari-gpu`
- #140 — Optimize high-upside `amari-gpu` kernels identified by crossover data
- #141 — Revisit `amari-gpu` coverage for upcoming crates and extensions
- #142 — Plan dedicated migration from `wgpu 0.19` to `wgpu 29`

`wgpu 29` migration note:

- track it during 0.23.0 as a dedicated migration effort
- allow it to slip to 0.24.0 if compile/API/runtime changes are too broad
- do not combine it with unrelated release work
- rerun GB10 and RTX 5080 validation before claiming the migration complete

Potential `amari-cli` GPU commands:

```text
amari gpu info
amari gpu validate
amari gpu benchmark
amari gpu calibrate
```

These commands are a natural home for hardware-aware dispatch and benchmark calibration work.

## Summary table

| Version | Theme | Primary crates/work | GPU posture |
|---------|-------|---------------------|-------------|
| 0.20.0 | GPU stabilization | `amari-gpu` hardening, validation, benchmark docs | Establish known-good conservative baseline |
| 0.21.0 | Algebra extension | `amari-tropical`, `amari-dual` | Defer broad GPU follow-up |
| 0.22.0 | New mathematical foundations | `amari-cgt`, `amari-surreal` | No GPU blocker |
| 0.23.0 | Tooling + GPU revisit | `amari-surcomplex`, `amari-cli`, GPU follow-up issues | Benchmark/calibration/backend migration cycle |
