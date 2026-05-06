# @justinelliottcobb/amari-wasm v0.22.0

**Unified Mathematical Computing Library with High-Precision WebAssembly Support**

[![npm version](https://badge.fury.io/js/%40justinelliottcobb%2Famari-wasm.svg)](https://www.npmjs.com/package/@justinelliottcobb/amari-wasm)
[![CI](https://github.com/justinelliottcobb/Amari/actions/workflows/ci.yml/badge.svg)](https://github.com/justinelliottcobb/Amari/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

Amari is a comprehensive mathematical computing library that brings advanced algebraic systems to JavaScript/TypeScript through WebAssembly. Features high-precision arithmetic for spacecraft orbital mechanics and relativistic physics calculations with pure Rust implementation and no native dependencies for universal deployment.

## Features

| Module | Crate | Since | Description |
|--------|-------|-------|-------------|
| [Geometric Algebra](docs/geometric-algebra.md) | amari-core | v0.1 | Multivectors, rotors, geometric products for 3D rotations and spatial transformations |
| [Tropical Algebra](docs/tropical-algebra.md) | amari-tropical | v0.9.3 | Max-plus semiring operations plus `0.21.0` ordinal-weighted optimization carriers below ε₀ |
| [Automatic Differentiation](docs/automatic-differentiation.md) | amari-dual | v0.9.3 | Forward-mode AD with `0.21.0` branch policies, multi-dual seeding, and fixed-size gradient wrappers |
| Combinatorial Game Theory | amari-cgt | v0.22.0 | Short normal-play games, cuts, outcomes, comparison, nimbers, and inspection helpers |
| Short Surreal Numbers | amari-surreal | v0.22.0 | Exact dyadic short-surreal arithmetic and conversion to/from numeric CGT games |
| [Cellular Automata](docs/cellular-automata.md) | amari-automata | v0.9.4 | Geometric cellular automata with multivector states |
| [Holographic Memory](docs/holographic-memory.md) | amari-fusion | v0.12.3 | Vector Symbolic Architecture for associative memory with binding and bundling |
| [Measure Theory](docs/measure-theory.md) | amari-measure | v0.10.0 | Lebesgue integration, probability measures, and measure-theoretic foundations |
| [Probability](docs/probability.md) | amari-probabilistic | v0.13.0 | Distributions on multivector spaces, MCMC sampling, Monte Carlo estimation |
| [Functional Analysis](docs/functional-analysis.md) | amari-functional | v0.15.0 | Hilbert spaces, linear operators, spectral decomposition, Sobolev spaces |
| [Optical Fields](docs/optical-fields.md) | amari-holographic | v0.15.1 | GA-native Lee hologram encoding for DMD displays and VSA-based optical processing |
| [Computational Topology](docs/topology.md) | amari-topology | v0.16.0 | Simplicial complexes, homology, persistent homology, Morse theory |
| [Dynamical Systems](docs/dynamics.md) | amari-calculus | v0.19.1 | ODE solvers, stability analysis, bifurcation diagrams, Lyapunov exponents |
| [Enumerative Geometry](docs/enumerative-geometry.md) | amari-enumerative | v0.19.1 | WDVV curve counting, matroids, CSM classes, stability conditions |
| [Probabilistic Contracts](docs/probabilistic-contracts.md) | amari-flynn | v0.19.1 | SMT-LIB2 proof obligations, Monte Carlo verification, rare event tracking |
| [GF(2) Algebra](docs/gf2-algebra.md) | amari-core, amari-enumerative | v0.19.1 | GF(2) linear algebra, binary Clifford algebra, coding theory, matroid representability, Kazhdan-Lusztig polynomials |
| [Orbital Mechanics](docs/orbital-mechanics.md) | amari-relativistic | v0.9.4 | Spacetime algebra (Cl(1,3)) with high-precision trajectory calculations |

Also includes bindings for: amari-network (geometric network analysis), amari-optimization (gradient descent, NSGA-II), amari-info-geom (Fisher metrics, statistical manifolds), amari-calculus (differential geometry, manifolds).

### v0.22.0 CGT / Surreal WASM Surface

The `0.22.0` release exposes the new short-game and short-surreal Rust APIs to JavaScript/TypeScript:

- `WasmCgtArena` and `WasmGameId` for arena-backed short games, focused one-option cuts, addition/subtraction/negation, comparison, outcomes, canonicalization, and formatting.
- `WasmGameInspection` for birthday, canonical form, outcome, numeric/impartial classification, and reachable-node counts.
- `WasmDyadic` for exact dyadic arithmetic with checked division inside the short-surreal layer.
- `WasmShortSurreal` for exact short-surreal arithmetic plus conversion from numeric CGT games and reconstruction back into a `WasmCgtArena`.

The scope remains intentionally short/exact: loopy games, misère play, symbolic infinite surreals, and surcomplex arithmetic are not part of the `0.22.0` WASM surface.

### v0.21.0 Tropical / Dual WASM Surface

The `0.21.0` release exposes the new optimization-oriented Rust APIs to JavaScript/TypeScript:

- `TropicalBatch.foldOplus(...)` and `TropicalBatch.foldOtimes(...)` for semiring folds.
- `WasmOrdinalArena`, `WasmOrdinal`, and `WasmOrdinalWeight` for bounded ordinals below ε₀, ordinal formatting/inspection, and ordinal-weight `oplus` / `otimes` helpers.
- `WasmBranchPolicy` plus `maxByPolicy(...)` / `minByPolicy(...)` on dual and multi-dual numbers for explicit tie behavior.
- `WasmMultiDualNumber.variables(...)` for basis-seeded gradient setup.
- `WasmStaticMultiDual2`, `WasmStaticMultiDual3`, and `WasmStaticMultiDual4` for small fixed-size forward-mode AD loops in browser-facing workloads.

### High-Precision Arithmetic

- **Pure Rust Backend**: dashu-powered arithmetic with no native dependencies
- **Universal Deployment**: Same precision guarantees across desktop, web, and edge environments
- **Orbital-Grade Tolerance**: Configurable precision for critical trajectory calculations

## Installation

```bash
npm install @justinelliottcobb/amari-wasm
```

Or with yarn:

```bash
yarn add @justinelliottcobb/amari-wasm
```

## Quick Start

```typescript
import init, { WasmMultivector, WasmRotor } from '@justinelliottcobb/amari-wasm';

async function main() {
  await init();

  // Create basis vectors
  const e1 = WasmMultivector.basis_vector(0);
  const e2 = WasmMultivector.basis_vector(1);

  // Compute geometric product
  const product = e1.geometric_product(e2);
  console.log(product.to_string()); // e12 (bivector)

  // Create a rotor for 90-degree rotation
  const rotor = WasmRotor.from_axis_angle(
    WasmMultivector.basis_vector(2),
    Math.PI / 2
  );

  // Rotate a vector
  const vector = WasmMultivector.from_coefficients(
    new Float64Array([1, 0, 0, 0, 0, 0, 0, 0])
  );
  const rotated = rotor.rotate_vector(vector);

  // Clean up WASM memory
  e1.free(); e2.free(); product.free();
  rotor.free(); vector.free(); rotated.free();
}

main();
```

See the [docs/](docs/) directory for detailed guides and API references for each module.

## Use Cases

- **Computer Graphics**: 3D rotations and transformations using rotors
- **Physics Simulations**: Geometric algebra for electromagnetic fields and relativistic calculations
- **Machine Learning**: Tropical neural networks and automatic differentiation
- **Optimization**: Tropical algebra for shortest path and scheduling problems
- **Scientific Computing**: High-performance mathematical operations with orbital-grade precision
- **Probability & Statistics**: Measure theory, numerical integration, and probabilistic modeling
- **Formal Verification**: SMT-LIB2 proof obligation generation for browser-based verification workflows
- **Symbolic AI**: Holographic memory for associative reasoning and concept binding
- **Holographic Displays**: Lee hologram encoding for DMD and SLM devices
- **Topological Data Analysis**: Persistent homology for shape and feature detection
- **Chaos Theory**: Lorenz attractors, bifurcation diagrams, Lyapunov exponents
- **Algebraic Geometry**: Rational curve counting, Schubert calculus, Gromov-Witten invariants
- **Error-Correcting Codes**: Binary linear codes (Hamming, Reed-Muller, Golay) for communication systems
- **Finite Field Combinatorics**: Grassmannian point counting, matroid representability, Kazhdan-Lusztig polynomials
- **Spacecraft Trajectory Planning**: High-precision orbital mechanics in web applications

## Building from Source

```bash
# Clone the repository
git clone https://github.com/justinelliottcobb/Amari.git
cd Amari/amari-wasm

# Install dependencies
npm install

# Build WASM module
npm run build

# Run tests
npm test
```

## Contributing

Contributions are welcome. Please see [CONTRIBUTING.md](https://github.com/justinelliottcobb/Amari/blob/master/CONTRIBUTING.md) for details.

## License

MIT License - see [LICENSE](https://github.com/justinelliottcobb/Amari/blob/master/LICENSE) for details.

## Acknowledgments

- Built with Rust and wasm-bindgen
- Inspired by geometric algebra libraries like GAViewer and Ganja.js
- Tropical algebra concepts from discrete mathematics

## Contact

- GitHub: [@justinelliottcobb](https://github.com/justinelliottcobb)
- NPM: [@justinelliottcobb/amari-wasm](https://www.npmjs.com/package/@justinelliottcobb/amari-wasm)

---

Made with Rust by the Amari team
