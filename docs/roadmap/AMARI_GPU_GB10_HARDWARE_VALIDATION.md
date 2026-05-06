# amari-gpu GB10 Hardware Validation

Date: 2026-04-28
Host: `great-attractor`
Platform: DGX Spark / NVIDIA GB10
OS/kernel: `Linux great-attractor 6.17.0-1014-nvidia #14-Ubuntu SMP PREEMPT_DYNAMIC Tue Mar 17 19:01:40 UTC 2026 aarch64`
Driver/CUDA from `nvidia-smi`: NVIDIA driver `580.142`, CUDA `13.0`
GPU query: `NVIDIA GB10, 580.142, 40C, P8, 2% util, ~5.41W` at validation time

## Scope

This pass validates the current `amari-gpu` 0.20.0 public API hardening work on actual GB10 hardware. It focuses on compile/runtime health, WebGPU context creation, focused public import-path/baseline tests, and the serial all-features suite.

It is **not yet** a benchmark/crossover report. Performance numbers and crossovers should be produced in a separate benchmark pass.

## Hardware discovery

```text
$ uname -a
Linux great-attractor 6.17.0-1014-nvidia #14-Ubuntu SMP PREEMPT_DYNAMIC Tue Mar 17 19:01:40 UTC 2026 aarch64 aarch64 aarch64 GNU/Linux

$ nvidia-smi
NVIDIA-SMI 580.142        Driver Version: 580.142        CUDA Version: 13.0
GPU 0: NVIDIA GB10
```

No special `WGPU_*`, `VK_*`, `CUDA_*`, `NVIDIA_*`, or `AMARI_*` environment overrides were set during the validation run. `XDG_RUNTIME_DIR=/run/user/1000` was present.

## Focused public API validation

All commands below passed on GB10.

| Domain / area | Command | Result |
|----------------|---------|--------|
| default core GA + info geometry | `cargo +stable test -p amari-gpu --test core_info_geometry_public_api -- --nocapture` | ✅ 3 passed |
| network | `cargo +stable test -p amari-gpu --test network_public_api -- --nocapture` | ✅ 3 passed |
| relativistic | `cargo +stable test -p amari-gpu --test relativistic_public_api -- --nocapture` | ✅ 2 passed |
| holographic | `cargo +stable test -p amari-gpu --features holographic --test holographic_public_api -- --nocapture` | ✅ 3 passed |
| tropical | `cargo +stable test -p amari-gpu --features tropical --test tropical_public_api -- --nocapture` | ✅ 2 passed |
| fusion | `cargo +stable test -p amari-gpu --features fusion --test fusion_public_api -- --nocapture` | ✅ 4 passed |
| calculus | `cargo +stable test -p amari-gpu --features calculus --test calculus_public_api -- --nocapture` | ✅ 2 passed |
| measure | `cargo +stable test -p amari-gpu --features measure --test measure_public_api -- --nocapture` | ✅ 1 passed |
| functional | `cargo +stable test -p amari-gpu --features functional --test functional_public_api -- --nocapture` | ✅ 1 passed |
| topology | `cargo +stable test -p amari-gpu --features topology --test topology_public_api -- --nocapture` | ✅ 1 passed |
| dual | `cargo +stable test -p amari-gpu --features dual --test dual_public_api -- --nocapture` | ✅ 1 passed |
| automata | `cargo +stable test -p amari-gpu --features automata --test automata_public_api -- --nocapture` | ✅ 1 passed |
| probabilistic | `cargo +stable test -p amari-gpu --features probabilistic --test probabilistic_public_api -- --nocapture` | ✅ 2 passed |
| GF(2) public API | `cargo +stable test -p amari-gpu --features gf2 --test gf2_public_api -- --nocapture` | ✅ 2 passed |
| GF(2) CPU parity | `cargo +stable test -p amari-gpu --features gf2 --test gf2_cpu_parity -- --nocapture` | ✅ 1 passed |
| enumerative | `cargo +stable test -p amari-gpu --features enumerative --test enumerative_public_api -- --nocapture` | ✅ 1 passed |
| infra/adaptive/performance/timeline | `cargo +stable test -p amari-gpu --test infra_public_api -- --nocapture` | ✅ 6 passed |

## Serial all-features validation

Command:

```bash
cargo +stable test -p amari-gpu --all-features --quiet -- --test-threads=1
```

Result: ✅ passed

Summary from run:

- library tests: `131 passed; 0 failed; 23 ignored`
- integration/public tests all passed
- final ignored-only target: `0 passed; 0 failed; 8 ignored`

Serial execution was used intentionally to avoid GPU context contention across feature-heavy test targets.

## Default validation and lint

Commands:

```bash
cargo +stable test -p amari-gpu --quiet
cargo +stable clippy -p amari-gpu -- -D warnings
```

Result: ✅ both passed

Default test summary:

- library tests: `53 passed; 0 failed; 7 ignored`
- integration tests all passed

## Interpretation

GB10 validation succeeded for the current 0.20.0 public API hardening surface:

- WebGPU context creation works on the DGX Spark / GB10 stack.
- Focused public import-path/baseline tests pass for default and feature-gated domains.
- Serial `--all-features` validation passes, including feature interactions.
- No headless EGL panic or shader validation regression was observed in this run.
- Previously risky areas now exercised successfully on hardware include:
  - signature-specific core GA shader basis counts
  - ProductCl3x32 holographic binding parity
  - optical bind pipeline under portable storage-buffer limits
  - probabilistic GPU statistics/sampling paths
  - GF(2) fixed-layout kernels
  - enumerative representative kernels
  - topology/functional/measure/automata mixed GPU/fallback paths

## Remaining hardware work

- Run the same focused and all-features validation on RTX 5080.
- Add benchmark/crossover reports for restored kernels:
  - tropical matmul/attention
  - core GA batch geometric product
  - holographic ProductCl3x32 bind/similarity
  - optical bind/similarity/Lee encoding
  - GF(2) kernels
  - probabilistic sampling/statistics
  - topology distance/Morse
  - automata rule/energy
  - measure built-ins
  - functional matrix batches
  - network distances/centrality/clustering
- Decide whether ignored heavy holographic/fusion tests should remain ignored, become hardware-only tests, or move to benchmark/manual validation.
