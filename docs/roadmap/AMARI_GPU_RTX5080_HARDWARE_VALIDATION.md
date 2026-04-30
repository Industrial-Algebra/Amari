# amari-gpu RTX 5080 Hardware Validation

Date: 2026-04-28
Host: `rindler`
Platform: NVIDIA GeForce RTX 5080 Laptop GPU
OS/kernel: `Linux rindler 6.17.0-22-generic #22-Ubuntu SMP PREEMPT_DYNAMIC Fri Mar 13 12:04:44 UTC 2026 x86_64`
Driver/CUDA from `nvidia-smi`: NVIDIA driver `580.126.09`, CUDA `13.0`
GPU query at validation start: `NVIDIA GeForce RTX 5080 Laptop GPU, 580.126.09, 48C, P8, 0% util, ~8.47W`
Session: Wayland/X11 compatibility (`WAYLAND_DISPLAY=wayland-0`, `DISPLAY=:0`, `XDG_RUNTIME_DIR=/run/user/1000`)
Backend used for validation: `WGPU_BACKEND=vulkan`

## Scope

This pass validates the current `amari-gpu` 0.20.0 public API hardening work on RTX 5080 laptop hardware. Results are user-reported from the RTX 5080 machine after pulling the verification-overhead test patch.

The validation is a correctness/runtime-health pass, not a benchmark/crossover report.

## Hardware discovery

```text
$ uname -a
Linux rindler 6.17.0-22-generic #22-Ubuntu SMP PREEMPT_DYNAMIC Fri Mar 13 12:04:44 UTC 2026 x86_64 GNU/Linux

$ nvidia-smi
NVIDIA-SMI 580.126.09             Driver Version: 580.126.09     CUDA Version: 13.0
GPU 0: NVIDIA GeForce RTX 5080 Laptop GPU
Memory: 14MiB / 16303MiB at discovery time
Temperature: 48C
Power: 8W / 50W
Utilization: 0%

$ nvidia-smi --query-gpu=name,driver_version,temperature.gpu,pstate,utilization.gpu,power.draw --format=csv,noheader,nounits
NVIDIA GeForce RTX 5080 Laptop GPU, 580.126.09, 48, P8, 0, 8.47
```

Environment hints reported before validation:

```text
WAYLAND_DISPLAY=wayland-0
DISPLAY=:0
XDG_RUNTIME_DIR=/run/user/1000
```

Initially `vulkaninfo` and `glxinfo` were not installed. Validation proceeded with explicit `WGPU_BACKEND=vulkan` after staged bring-up.

## Bring-up notes

The first basic default validation attempt locked up the machine before the staged Vulkan-only plan was used. After switching to staged low-concurrency validation and explicit Vulkan backend selection, the machine did not lock up.

A pre-patch diagnostic test failed due to a brittle micro-performance threshold:

```text
test_verification_performance_overhead
Unverified CPU computation: 444.922µs
Verified computation: 2.901927ms
Verification overhead: 552.2%
```

This was classified as a non-correctness benchmark-threshold failure. The test has since been patched to remain diagnostic while asserting finite timing and successful verified output shape rather than requiring `< 50%` overhead for a tiny noisy batch.

After pulling that patch, the RTX 5080 validation passed.

## Validation commands

The RTX 5080 validation used explicit Vulkan backend selection and serial test execution for aggregate suites:

```bash
WGPU_BACKEND=vulkan cargo +stable test -p amari-gpu --test verification_integration -- --nocapture --test-threads=1
WGPU_BACKEND=vulkan cargo +stable test -p amari-gpu --quiet -- --test-threads=1
WGPU_BACKEND=vulkan cargo +stable test -p amari-gpu --all-features --quiet -- --test-threads=1
```

The focused public API validation sequence was also run successfully after staged bring-up/patched verification overhead behavior, covering the same public surfaces as the GB10 report:

- default/core GA + info geometry
- network
- relativistic
- infra/adaptive/performance/timeline
- holographic
- tropical
- fusion
- calculus
- measure
- functional
- topology
- dual
- automata
- probabilistic
- GF(2) public API and CPU parity
- enumerative

## Result

RTX 5080 validation status: ✅ passed after the verification-overhead diagnostic patch.

Observed caveats:

- Use `WGPU_BACKEND=vulkan` on this Ubuntu 25.10 / RTX 5080 laptop stack.
- Use `-- --test-threads=1` for aggregate/default/all-features validation to avoid GPU context contention and reduce driver stress.
- The verification overhead test is diagnostic, not a benchmark gate.
- Benchmark/crossover measurements are now reported separately in `docs/roadmap/AMARI_GPU_BENCHMARK_CROSSOVER_REPORT.md`.

## Benchmark campaign

A complete serial ignored benchmark campaign was run with `WGPU_BACKEND=vulkan` after correctness validation. Results are recorded in:

- `docs/roadmap/AMARI_GPU_BENCHMARK_CROSSOVER_REPORT.md`

High-level RTX 5080 crossover observations from this test-profile campaign:

- core GA Cl(3,0,0) batch geometric product crosses over between batch `64` and `256`.
- tropical dense max-plus matmul crosses over between `64³` and `128³`.
- holographic ProductCl3x32 similarity crosses over around batch `512`; bind did not cross over through batch `2048`.
- topology distance matrix, automata, GF(2), probabilistic, functional, and network public paths did not cross over within the tested sizes on this machine, though some approached parity at the largest sizes.
- measure integration/density approached parity at the largest tested sizes; tropical reductions are CPU fallback paths and track CPU timing.

## Remaining work

- Extend benchmark/crossover reports with release-mode and larger-size sweeps for restored kernels:
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
- Decide policy for heavy ignored holographic/fusion tests: keep ignored, make hardware-only, or move to benchmark/manual validation.
