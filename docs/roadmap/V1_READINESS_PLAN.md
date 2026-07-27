# Amari v1.0.0 Readiness Plan

## Executive Summary

Amari has completed all core analytical foundations (v0.15.0 - v0.17.0). This document outlines the remaining work required for a stable 1.0.0 release.

**Current Status**: v0.17.0 released
**Target**: v1.0.0 stable release
**Blockers**: Test coverage gaps, documentation gaps, benchmark coverage

---

## 1. Test Coverage Assessment

### Current State

| Metric | Current | Target | Gap |
|--------|---------|--------|-----|
| Total Tests | 1,637 | 2,000+ | +363 |
| Ignored Tests | 121 | <20 | -101 |
| Test Failures | 0 | 0 | Met |

### Coverage by Crate

#### Critical Gaps (Blocking 1.0.0)

| Crate | Src LOC | Tests | Status | Action Required |
|-------|---------|-------|--------|-----------------|
| **amari-wasm** | 8,006 | 3 | CRITICAL | Add 50+ comprehensive WASM binding tests |
| **amari-automata** | 2,942 | 1 | CRITICAL | Add 30+ cellular automata tests |
| **amari-gpu** | 17,561 | 65 | LOW | Add 50+ GPU operation tests |

#### Adequate Coverage (OK for 1.0.0)

| Crate | Src LOC | Tests | Status |
|-------|---------|-------|--------|
| amari-dynamics | 14,454 | 286 | Excellent |
| amari-measure | 3,675 | 211 | Excellent |
| amari-core | 4,188 | 198 | Good |
| amari-holographic | 4,930 | 155 | Good |
| amari-enumerative | 3,451 | 144 | Good |
| amari-functional | 3,059 | 75 | Adequate |
| amari-relativistic | 3,208 | 69 | Adequate |
| amari-calculus | 1,864 | 67 | Adequate |
| amari-topology | 2,967 | 63 | Adequate |
| amari-probabilistic | 2,591 | 54 | Adequate |
| amari-info-geom | 1,894 | 53 | Adequate |
| amari-fusion | 3,153 | 42 | Adequate |
| amari-optimization | 2,773 | 42 | Adequate |

### Test Plan

#### Phase 1: Critical Test Additions (Week 1-2)

**amari-wasm (Target: 50+ tests)**
```
tests/
  multivector_bindings.rs    # 15 tests - all Multivector methods
  dual_bindings.rs           # 10 tests - DualNumber operations
  tropical_bindings.rs       # 10 tests - TropicalNumber operations
  topology_bindings.rs       # 5 tests - Topology WASM interface
  integration_tests.rs       # 10 tests - Cross-type operations
```

**amari-automata (Target: 30+ tests)**
```
tests/
  cellular_automaton.rs      # 10 tests - Rule evaluation
  geometric_rules.rs         # 10 tests - GA-based rule systems
  evolution.rs               # 10 tests - Time evolution
```

**amari-gpu (Target: 50+ additional tests)**
```
tests/
  shader_compilation.rs      # 10 tests - All shaders compile
  cpu_gpu_parity.rs          # 20 tests - CPU/GPU results match
  memory_management.rs       # 10 tests - Buffer handling
  error_handling.rs          # 10 tests - Graceful degradation
```

#### Phase 2: Ignored Test Review (Week 2)

Review all 121 ignored tests:
- GPU-dependent tests: Add CI skip markers or mock GPU context
- Platform-specific tests: Add proper cfg attributes
- Flaky tests: Fix or document known issues
- Obsolete tests: Remove if no longer relevant

---

## 2. Documentation Assessment

### Current State

| Crate | Pub Items | Documented | Coverage |
|-------|-----------|------------|----------|
| amari-relativistic | 101 | 64 | 63% |
| amari-optimization | 60 | 32 | 53% |
| amari-holographic | 86 | 36 | 41% |
| amari-measure | 124 | 51 | 41% |
| amari-core | 75 | 30 | 40% |
| amari-calculus | 47 | 19 | 40% |
| amari-dual | 48 | 18 | 37% |
| amari-info-geom | 28 | 10 | 35% |
| amari-functional | 94 | 32 | 34% |
| amari-flynn | 30 | 10 | 33% |
| amari-gpu | 219 | 70 | 31% |
| amari-dynamics | 316 | 95 | 30% |
| amari-probabilistic | 73 | 20 | 27% |
| amari-network | 31 | 8 | 25% |
| amari-topology | 99 | 24 | 24% |
| amari-automata | 87 | 19 | 21% |
| amari-fusion | 50 | 10 | 20% |
| amari-tropical | 30 | 6 | 20% |
| amari-enumerative | 91 | 8 | 8% |
| **amari-wasm** | **129** | **1** | **0%** |

### Documentation Plan

#### Priority 1: Critical Crates (0-20% coverage)

1. **amari-wasm** (0% → 80%)
   - Document all public WASM bindings
   - Add JavaScript/TypeScript usage examples
   - Document memory management requirements

2. **amari-enumerative** (8% → 80%)
   - Document algebraic curve types
   - Add mathematical background
   - Include worked examples

3. **amari-tropical** (20% → 80%)
   - Document TropicalNumber API
   - Add max-plus algebra examples
   - Include optimization use cases

4. **amari-fusion** (20% → 80%)
   - Document TropicalDualClifford
   - Add fusion system examples

5. **amari-automata** (21% → 80%)
   - Document cellular automata API
   - Add rule definition examples

#### Priority 2: Important Crates (20-40% coverage)

- amari-topology, amari-network, amari-probabilistic
- amari-dynamics, amari-gpu, amari-functional

#### Priority 3: Adequate Crates (40%+ coverage)

- Improve existing docs, add examples
- Focus on complex APIs

---

## 3. Performance Audit

### Benchmark Coverage

| Status | Crate | Benchmarks |
|--------|-------|------------|
| ✅ | amari-core | performance_suite.rs |
| ✅ | amari-measure | integration_benchmarks.rs |
| ✅ | amari-optimization | optimization_benchmarks.rs |
| ✅ | amari-relativistic | precision_benchmarks.rs |
| ✅ | amari-fusion | comparison.rs |
| ❌ | amari-dual | None |
| ❌ | amari-tropical | None |
| ❌ | amari-calculus | None |
| ❌ | amari-probabilistic | None |
| ❌ | amari-functional | None |
| ❌ | amari-topology | None |
| ❌ | amari-dynamics | None |
| ❌ | amari-holographic | None |
| ❌ | amari-gpu | None |

### Current Performance Baseline (amari-core)

| Operation | Time | Throughput |
|-----------|------|------------|
| Geometric product (scalar) | 44 ns | 22.7 Mop/s |
| Geometric product (SIMD) | 44 ns | 22.7 Mop/s |
| Complex multivector product | 52 ns | 19.2 Mop/s |
| Batch operations (10k) | 540 µs | 18.5 Melem/s |
| Rotor creation | 206 ns | 4.9 Mop/s |
| Rotor application | 110 ns | 9.1 Mop/s |

### Required Benchmarks for 1.0.0

#### Core Mathematical Operations

```rust
// amari-dual/benches/dual_benchmarks.rs
- dual_arithmetic (add, mul, div)
- gradient_computation (1-var, multi-var)
- chain_rule_application

// amari-tropical/benches/tropical_benchmarks.rs
- tropical_arithmetic (add, mul)
- tropical_matrix_operations
- shortest_path_computation

// amari-calculus/benches/calculus_benchmarks.rs
- gradient_computation
- divergence_curl
- vector_derivative
```

#### New Analytical Crates

```rust
// amari-functional/benches/functional_benchmarks.rs
- inner_product_computation
- operator_application
- spectral_decomposition
- eigenvalue_computation

// amari-topology/benches/topology_benchmarks.rs
- simplicial_complex_construction
- boundary_operator
- homology_computation
- persistent_homology (varying sizes)

// amari-dynamics/benches/dynamics_benchmarks.rs
- rk4_integration (single step, trajectory)
- stability_analysis
- lyapunov_computation
- bifurcation_diagram
```

#### GPU Operations

```rust
// amari-gpu/benches/gpu_benchmarks.rs
- cpu_vs_gpu_crossover_point
- batch_geometric_product
- parallel_trajectory_integration
- matrix_operations
```

### Performance Targets

| Category | Target | Baseline Comparison |
|----------|--------|---------------------|
| GA operations | < 100ns per op | Competitive with ganja.js |
| Batch operations | > 10M elem/s | Linear scaling |
| ODE integration | < 1µs per step | Competitive with scipy |
| GPU crossover | > 1000 elements | Automatic dispatch |
| WASM overhead | < 2x native | Acceptable for web |

---

## 4. Implementation Timeline

### Week 1-2: Critical Tests

- [ ] amari-wasm: Add 50+ tests
- [ ] amari-automata: Add 30+ tests
- [ ] amari-gpu: Add 50+ tests
- [ ] Review ignored tests

### Week 3-4: Documentation Sprint

- [ ] amari-wasm: Full documentation
- [ ] amari-enumerative: Full documentation
- [ ] amari-tropical: Full documentation
- [ ] amari-automata: Full documentation
- [ ] amari-fusion: Full documentation

### Week 5-6: Benchmarks

- [ ] Add benchmarks to 9 crates without them
- [ ] Establish performance baselines
- [ ] Document performance characteristics
- [ ] Identify optimization opportunities

### Week 7-8: Polish & Release

- [ ] API review and deprecations
- [ ] CHANGELOG update
- [ ] Migration guide from 0.x
- [ ] Final CI/CD verification
- [ ] Run a dedicated 1.0.0-polish spike to conform legacy publish workflows to current IA release doctrine, including explicit feature matrices and artifact evidence
- [ ] Version bump and publish

---

## 5. Success Criteria for 1.0.0

### Testing
- [ ] All tests pass (0 failures)
- [ ] < 20 ignored tests (with documented reasons)
- [ ] 2,000+ total tests
- [ ] No critical crate with < 20 tests

### Documentation
- [ ] All public items documented (100%)
- [ ] All crates have README with examples
- [ ] API reference complete on docs.rs
- [ ] Migration guide available

### Performance
- [ ] All major crates have benchmarks
- [ ] Performance baselines documented
- [ ] No regressions from 0.17.0
- [ ] GPU crossover points validated

### Stability
- [ ] API stability guarantees documented
- [ ] Deprecation policy established
- [ ] SemVer compliance verified
- [ ] No breaking changes without major version

---

## 6. Risk Assessment

### High Risk
- **amari-wasm test coverage**: 8K LOC with 3 tests is a significant blind spot
- **GPU testing in CI**: Many tests ignored due to GPU requirements

### Medium Risk
- **Documentation debt**: Some crates at 0-20% coverage
- **Benchmark coverage**: 9 crates without performance data

### Low Risk
- **Core functionality**: Well-tested with 1,637 passing tests
- **New crates (dynamics, topology, functional)**: Good initial coverage

---

## Appendix: File Checklist

### Tests to Create

```
amari-wasm/tests/
  multivector_bindings.rs
  dual_bindings.rs
  tropical_bindings.rs
  topology_bindings.rs
  integration_tests.rs

amari-automata/tests/
  cellular_automaton.rs
  geometric_rules.rs
  evolution.rs

amari-gpu/tests/
  shader_compilation.rs
  cpu_gpu_parity.rs
  memory_management.rs
  error_handling.rs
```

### Benchmarks to Create

```
amari-dual/benches/dual_benchmarks.rs
amari-tropical/benches/tropical_benchmarks.rs
amari-calculus/benches/calculus_benchmarks.rs
amari-probabilistic/benches/probabilistic_benchmarks.rs
amari-functional/benches/functional_benchmarks.rs
amari-topology/benches/topology_benchmarks.rs
amari-dynamics/benches/dynamics_benchmarks.rs
amari-holographic/benches/holographic_benchmarks.rs
amari-gpu/benches/gpu_benchmarks.rs
```

---

*Generated: 2026-01-13*
*Target Release: v1.0.0*
