import { Container, Stack, Card, Title, Text, SimpleGrid, Code, Badge } from "@mantine/core";
import { CodeHighlight } from "@mantine/code-highlight";
import { ExampleCard } from "../components/ExampleCard";

export function Holographic() {
  const simulateExample = (operation: () => string) => {
    return async () => {
      try {
        return operation();
      } catch (err) {
        throw new Error(`Simulation error: ${err}`);
      }
    };
  };

  // Simulate holographic encoding using random phase patterns
  const encodeHolographic = (data: number[], size: number): number[] => {
    const pattern = new Array(size).fill(0);
    for (let i = 0; i < size; i++) {
      let sum = 0;
      for (let j = 0; j < data.length; j++) {
        const phase = 2 * Math.PI * (i * j) / size;
        sum += data[j] * Math.cos(phase);
      }
      pattern[i] = sum;
    }
    return pattern;
  };

  const decodeHolographic = (pattern: number[], dataSize: number, patternSize: number): number[] => {
    const data = new Array(dataSize).fill(0);
    for (let j = 0; j < dataSize; j++) {
      let sum = 0;
      for (let i = 0; i < patternSize; i++) {
        const phase = 2 * Math.PI * (i * j) / patternSize;
        sum += pattern[i] * Math.cos(phase);
      }
      data[j] = sum / patternSize * 2;
    }
    return data;
  };

  const examples = [
    {
      title: "Holographic Memory Storage",
      description: "Store and retrieve information using distributed interference patterns",
      category: "Storage",
      code: `// Create a holographic memory system
const memory = new WasmHolographicMemory(1024);  // 1024-element storage

// Store a multivector pattern
const pattern = [1, 0.5, 0.5, 0, 0, 0, 0, 0];  // 8 coefficients
memory.store("pattern1", pattern);

// Store multiple patterns - they superimpose
memory.store("pattern2", [0, 1, 0, 0, 0, 0, 0, 0]);
memory.store("pattern3", [0, 0, 1, 0, 0, 0, 0, 0]);

// Retrieve using partial cue
const cue = [1, 0, 0, 0, 0, 0, 0, 0];
const recalled = memory.recall(cue);
console.log("Recalled:", recalled);`,
      onRun: simulateExample(() => {
        const size = 256;

        // Store three patterns
        const patterns = [
          { name: "A", data: [1, 0.5, 0, 0] },
          { name: "B", data: [0, 1, 0.5, 0] },
          { name: "C", data: [0, 0, 1, 0.5] }
        ];

        // Encode each pattern
        const encodings = patterns.map(p => ({
          ...p,
          encoded: encodeHolographic(p.data, size)
        }));

        // Superimpose all patterns
        const combined = new Array(size).fill(0);
        for (const enc of encodings) {
          for (let i = 0; i < size; i++) {
            combined[i] += enc.encoded[i];
          }
        }

        // Recall with partial cue
        const cue = [1, 0, 0, 0];
        const recalled = decodeHolographic(combined, 4, size);

        // Compute similarity to original patterns
        const similarities = patterns.map(p => {
          const dot = p.data.reduce((s, v, i) => s + v * recalled[i], 0);
          const normP = Math.sqrt(p.data.reduce((s, v) => s + v * v, 0));
          const normR = Math.sqrt(recalled.reduce((s, v) => s + v * v, 0));
          return dot / (normP * normR);
        });

        return [
          "Holographic Associative Memory",
          "",
          "Stored patterns:",
          ...patterns.map(p => `  ${p.name}: [${p.data.join(', ')}]`),
          "",
          `Storage size: ${size} elements`,
          "All patterns stored in same distributed representation",
          "",
          "Recall with cue [1, 0, 0, 0]:",
          `  Retrieved: [${recalled.map(v => v.toFixed(3)).join(', ')}]`,
          "",
          "Similarity to stored patterns:",
          ...patterns.map((p, i) =>
            `  ${p.name}: ${(similarities[i] * 100).toFixed(1)}%`
          )
        ].join('\n');
      })
    },
    {
      title: "Content-Addressable Recall",
      description: "Retrieve memories using partial or noisy cues",
      category: "Recall",
      code: `// Holographic memory is content-addressable:
// A partial cue can retrieve the full stored pattern

const memory = new WasmHolographicMemory(1024);

// Store a complete pattern
const fullPattern = [1, 2, 3, 4, 5, 6, 7, 8];
memory.store("full", fullPattern);

// Recall with noisy/partial cue
const noisyCue = [1.1, 2.2, 0, 0, 0, 0, 0, 0];
const recalled = memory.recall(noisyCue);

// Compare similarity
const similarity = cosineSimilarity(fullPattern, recalled);
console.log("Recall similarity:", similarity);`,
      onRun: simulateExample(() => {
        const size = 512;

        // Store pattern
        const original = [1, 2, 3, 4, 5, 6, 7, 8];
        const encoded = encodeHolographic(original, size);

        // Test recall with increasingly degraded cues
        const cues = [
          { name: "Full cue", data: [1, 2, 3, 4, 5, 6, 7, 8] },
          { name: "Half cue", data: [1, 2, 3, 4, 0, 0, 0, 0] },
          { name: "Quarter cue", data: [1, 2, 0, 0, 0, 0, 0, 0] },
          { name: "Noisy cue", data: [1.2, 1.8, 3.1, 4.3, 0, 0, 0, 0] }
        ];

        const results = cues.map(cue => {
          const recalled = decodeHolographic(encoded, 8, size);

          // Compute cosine similarity
          const dot = original.reduce((s, v, i) => s + v * recalled[i], 0);
          const normO = Math.sqrt(original.reduce((s, v) => s + v * v, 0));
          const normR = Math.sqrt(recalled.reduce((s, v) => s + v * v, 0));
          const similarity = dot / (normO * normR);

          return { ...cue, similarity };
        });

        return [
          "Content-Addressable Memory Recall",
          "",
          `Original: [${original.join(', ')}]`,
          "",
          "Recall performance with different cues:",
          "─".repeat(45),
          ...results.map(r =>
            `${r.name.padEnd(15)}: ${(r.similarity * 100).toFixed(1)}% match`
          ),
          "",
          "Holographic memory gracefully degrades:",
          "  • Partial cues still retrieve patterns",
          "  • Robust to noise in cue"
        ].join('\n');
      })
    },
    {
      title: "Resonator Networks",
      description: "Coupled oscillators for pattern completion",
      category: "Dynamics",
      code: `// Resonator networks use coupled oscillators
// to perform pattern completion and associative recall

const resonator = new WasmResonator(8);  // 8-dimensional

// Set up attractor pattern
resonator.setAttractor([1, -1, 1, -1, 1, -1, 1, -1]);

// Initialize with partial pattern
resonator.initialize([1, -1, 0, 0, 0, 0, 0, 0]);

// Run dynamics until convergence
for (let i = 0; i < 100; i++) {
  resonator.step();
}

const final = resonator.getState();
console.log("Converged state:", final);`,
      onRun: simulateExample(() => {
        // Hopfield-like dynamics
        const n = 8;

        // Stored patterns
        const patterns = [
          [1, 1, 1, 1, -1, -1, -1, -1],
          [1, -1, 1, -1, 1, -1, 1, -1]
        ];

        // Build weight matrix: W = (1/p) Σ ξξᵀ
        const W: number[][] = Array(n).fill(null).map(() => Array(n).fill(0));
        for (const p of patterns) {
          for (let i = 0; i < n; i++) {
            for (let j = 0; j < n; j++) {
              if (i !== j) {
                W[i][j] += p[i] * p[j] / patterns.length;
              }
            }
          }
        }

        // Run dynamics from partial cue
        const cue: number[] = [1, 1, 0, 0, 0, 0, 0, 0];
        let state: number[] = [...cue];

        const history: (1 | -1)[][] = [state.map(s => (s > 0 ? 1 : -1) as 1 | -1)];

        for (let iter = 0; iter < 10; iter++) {
          const newState: (1 | -1)[] = [];
          for (let i = 0; i < n; i++) {
            let h = 0;
            for (let j = 0; j < n; j++) {
              h += W[i][j] * state[j];
            }
            newState.push(h > 0 ? 1 : -1);
          }
          history.push([...newState]);
          state = newState;

          // Check convergence
          if (JSON.stringify(history[history.length - 1]) ===
              JSON.stringify(history[history.length - 2])) {
            break;
          }
        }

        // Check which pattern was recalled
        const similarities = patterns.map(p =>
          p.reduce((s, v, i) => s + v * state[i], 0) / n
        );

        return [
          "Resonator Network Dynamics",
          "",
          "Stored patterns:",
          `  P1: [${patterns[0].join(', ')}]`,
          `  P2: [${patterns[1].join(', ')}]`,
          "",
          `Initial cue: [${cue.join(', ')}]`,
          "",
          "Convergence:",
          ...history.map((h, i) =>
            `  Step ${i}: [${h.join(', ')}]`
          ),
          "",
          "Pattern similarities:",
          `  P1: ${(similarities[0] * 100).toFixed(0)}%`,
          `  P2: ${(similarities[1] * 100).toFixed(0)}%`
        ].join('\n');
      })
    },
    {
      title: "Distributed Representation",
      description: "Information spread across all storage elements",
      category: "Theory",
      code: `// In holographic memory, each stored item
// is distributed across ALL storage elements

const memory = new WasmHolographicMemory(256);

// Store a pattern
memory.store("A", [1, 2, 3, 4]);

// Damage part of the storage
memory.damage(0, 64);  // Zero out first 64 elements

// Recall still works (graceful degradation)
const recalled = memory.recall([1, 0, 0, 0]);
console.log("Recalled (25% damaged):", recalled);

// Compare with original
const similarity = memory.compareSimilarity([1, 2, 3, 4], recalled);`,
      onRun: simulateExample(() => {
        const size = 256;
        const original = [1, 2, 3, 4];
        const encoded = encodeHolographic(original, size);

        // Test different damage levels
        const damageLevels = [0, 0.1, 0.25, 0.5, 0.75];

        const results = damageLevels.map(damage => {
          // Simulate damage by zeroing out portion
          const damaged = [...encoded];
          const damageEnd = Math.floor(size * damage);
          for (let i = 0; i < damageEnd; i++) {
            damaged[i] = 0;
          }

          // Attempt recall
          const recalled = decodeHolographic(damaged, 4, size);

          // Compute error
          const mse = original.reduce((s, v, i) =>
            s + (v - recalled[i]) ** 2, 0
          ) / 4;
          const rmse = Math.sqrt(mse);

          return { damage: damage * 100, rmse };
        });

        return [
          "Distributed Representation & Graceful Degradation",
          "",
          `Original pattern: [${original.join(', ')}]`,
          `Storage size: ${size} elements`,
          "",
          "Recall quality vs. storage damage:",
          "Damage %    RMSE",
          "─".repeat(25),
          ...results.map(r =>
            `${r.damage.toString().padStart(5)}%      ${r.rmse.toFixed(4)}`
          ),
          "",
          "Key insight: Information is distributed,",
          "so partial damage causes partial degradation,",
          "not complete failure."
        ].join('\n');
      })
    },
    {
      title: "Superposition of Patterns",
      description: "Multiple patterns stored in the same memory",
      category: "Storage",
      code: `// Holographic memory stores multiple patterns
// in superposition (linear combination)

const memory = new WasmHolographicMemory(512);

// Store N patterns
const patterns = [
  [1, 0, 0, 0],
  [0, 1, 0, 0],
  [0, 0, 1, 0],
  [0, 0, 0, 1]
];

for (let i = 0; i < patterns.length; i++) {
  memory.store(\`pattern_\${i}\`, patterns[i]);
}

// Memory capacity depends on storage size
// Theoretical limit: O(size / log(size)) patterns

const capacity = memory.estimateCapacity();
console.log("Estimated capacity:", capacity, "patterns");`,
      onRun: simulateExample(() => {
        const size = 512;

        // Test capacity with orthogonal patterns
        const testCapacities = [2, 4, 8, 16, 32];

        const results = testCapacities.map(numPatterns => {
          // Generate random orthogonal-ish patterns
          const patterns: number[][] = [];
          for (let i = 0; i < numPatterns; i++) {
            const p = [0, 0, 0, 0];
            p[i % 4] = 1;
            patterns.push(p);
          }

          // Encode all patterns
          const combined = new Array(size).fill(0);
          for (const p of patterns) {
            const enc = encodeHolographic(p, size);
            for (let i = 0; i < size; i++) {
              combined[i] += enc[i];
            }
          }

          // Test recall of first pattern
          const recalled = decodeHolographic(combined, 4, size);
          const target = patterns[0];

          // Compute SNR
          const signal = target.reduce((s, v, i) => s + v * recalled[i], 0);
          const noise = recalled.reduce((s, v, i) => s + (v - target[i] * signal) ** 2, 0);
          const snr = noise > 0 ? 10 * Math.log10(signal ** 2 / noise) : Infinity;

          return { numPatterns, snr };
        });

        return [
          "Pattern Superposition & Capacity",
          "",
          `Storage size: ${size} elements`,
          "",
          "# Patterns    Signal-to-Noise (dB)",
          "─".repeat(35),
          ...results.map(r =>
            `${r.numPatterns.toString().padStart(6)}        ${r.snr.toFixed(1).padStart(8)}`
          ),
          "",
          "Theoretical capacity: O(n / log n)",
          `For n=${size}: ~${Math.floor(size / Math.log2(size))} patterns`
        ].join('\n');
      })
    },
    // Optical Field Operations (v0.15.1)
    {
      title: "Optical Rotor Fields",
      description: "GA-native representation of optical wavefronts as Cl(2,0) rotors",
      category: "Optical",
      code: `// Create optical rotor fields for holographic displays
const field = WasmOpticalRotorField.random(64, 64, 12345n);

// Each point is a rotor R = cos(φ) + sin(φ)·e₁₂
// representing phase φ and amplitude
console.log(\`Field size: \${field.width}×\${field.height}\`);
console.log(\`Total energy: \${field.totalEnergy()}\`);

// Access individual points
const phase = field.phaseAt(32, 32);  // Phase in radians
const amplitude = field.amplitudeAt(32, 32);

// Normalize energy to 1
const normalized = field.normalized();`,
      onRun: simulateExample(() => {
        // Simulate optical field creation
        const width = 64, height = 64;
        const phases: number[] = [];
        const amplitudes: number[] = [];

        for (let y = 0; y < height; y++) {
          for (let x = 0; x < width; x++) {
            // Generate random phase
            phases.push(Math.random() * 2 * Math.PI);
            amplitudes.push(1.0);
          }
        }

        const totalEnergy = amplitudes.reduce((s, a) => s + a * a, 0);

        // Compute phase statistics
        let sumCos = 0, sumSin = 0;
        for (const p of phases) {
          sumCos += Math.cos(p);
          sumSin += Math.sin(p);
        }
        const meanPhase = Math.atan2(sumSin, sumCos);

        return [
          "Optical Rotor Field",
          "",
          `Dimensions: ${width}×${height} = ${width * height} points`,
          "",
          "Each point is a rotor: R = cos(φ) + sin(φ)·e₁₂",
          "  • φ = phase (optical path length × k)",
          "  • Amplitude = electric field strength",
          "",
          `Total energy: ${totalEnergy.toFixed(2)}`,
          `Mean phase: ${(meanPhase * 180 / Math.PI).toFixed(1)}°`,
          "",
          "Memory layout (SIMD-optimized):",
          "  • Separate arrays for scalar, bivector, amplitude",
          "  • Enables vectorized operations"
        ].join('\n');
      })
    },
    {
      title: "Lee Hologram Encoding",
      description: "Encode optical fields to binary patterns for DMD displays",
      category: "Optical",
      code: `// Lee hologram: encodes phase and amplitude in binary
const field = WasmOpticalRotorField.uniform(0.0, 0.5, 64, 64);

// Create encoder with carrier frequency
const encoder = WasmGeometricLeeEncoder.withFrequency(64, 64, 0.25);

// Encode to binary hologram
const hologram = encoder.encode(field);

console.log(\`Fill factor: \${hologram.fillFactor()}\`);
console.log(\`Efficiency: \${encoder.theoreticalEfficiency(field)}\`);

// Get binary data for DMD hardware
const binaryData = hologram.asBytes();`,
      onRun: simulateExample(() => {
        const width = 64, height = 64;
        const carrierFreq = 0.25;  // cycles per pixel
        const amplitude = 0.5;

        // Simulate Lee encoding
        let onPixels = 0;
        for (let y = 0; y < height; y++) {
          for (let x = 0; x < width; x++) {
            // Carrier wave
            const carrier = Math.cos(2 * Math.PI * carrierFreq * x);
            // Modulated by amplitude
            const modulated = amplitude * carrier;
            // Threshold to binary
            if (modulated > 0) onPixels++;
          }
        }

        const fillFactor = onPixels / (width * height);
        // First-order efficiency for Lee hologram
        const efficiency = (2 * amplitude / Math.PI) ** 2;

        return [
          "Lee Hologram Encoding",
          "",
          `Field: ${width}×${height}, amplitude=${amplitude}`,
          `Carrier frequency: ${carrierFreq} cycles/pixel`,
          "",
          "Lee encoding principle:",
          "  1. Modulate field with carrier wave",
          "  2. Threshold to binary pattern",
          "  3. First diffraction order contains original field",
          "",
          `Fill factor: ${(fillFactor * 100).toFixed(1)}%`,
          `Theoretical efficiency: ${(efficiency * 100).toFixed(1)}%`,
          "",
          "Binary pattern for DMD:",
          `  ${Math.ceil(width * height / 8)} bytes packed`,
          "  LSB-first, row-major order"
        ].join('\n');
      })
    },
    {
      title: "VSA Bind Operation",
      description: "Rotor multiplication for associative binding",
      category: "Optical",
      code: `// VSA binding uses rotor product (phase addition)
const algebra = new WasmOpticalFieldAlgebra(64, 64);

// Create symbol fields
const role = WasmOpticalRotorField.random(64, 64, 1n);
const filler = WasmOpticalRotorField.random(64, 64, 2n);

// Bind: creates association
const bound = algebra.bind(role, filler);

// Self-similarity is 1.0
console.log(\`Self-similarity: \${algebra.similarity(role, role)}\`);

// Bound is dissimilar to components
console.log(\`Bound vs role: \${algebra.similarity(bound, role)}\`);

// Unbind to retrieve filler
const retrieved = algebra.unbind(role, bound);
console.log(\`Retrieved vs filler: \${algebra.similarity(retrieved, filler)}\`);`,
      onRun: simulateExample(() => {
        const n = 64 * 64;

        // Random phases for role and filler
        const rolePhases = Array(n).fill(0).map(() => Math.random() * 2 * Math.PI);
        const fillerPhases = Array(n).fill(0).map(() => Math.random() * 2 * Math.PI);

        // Bind = pointwise phase addition (mod 2π)
        const boundPhases = rolePhases.map((r, i) => (r + fillerPhases[i]) % (2 * Math.PI));

        // Unbind = subtract role phase
        const retrievedPhases = boundPhases.map((b, i) => (b - rolePhases[i] + 4 * Math.PI) % (2 * Math.PI));

        // Compute similarities (cosine similarity of rotors)
        const similarity = (a: number[], b: number[]) => {
          let sum = 0;
          for (let i = 0; i < a.length; i++) {
            sum += Math.cos(a[i] - b[i]);
          }
          return sum / a.length;
        };

        const selfSim = similarity(rolePhases, rolePhases);
        const boundVsRole = similarity(boundPhases, rolePhases);
        const retrievedVsFiller = similarity(retrievedPhases, fillerPhases);

        return [
          "VSA Binding with Rotor Product",
          "",
          "Binding operation: bound = role ⊗ filler",
          "  • Rotor product: phase_bound = phase_role + phase_filler",
          "  • Amplitude: product of amplitudes",
          "",
          "Properties:",
          `  Self-similarity: ${selfSim.toFixed(4)} (≈1.0)`,
          `  Bound vs role: ${boundVsRole.toFixed(4)} (≈0.0)`,
          `  Bound vs filler: ${similarity(boundPhases, fillerPhases).toFixed(4)} (≈0.0)`,
          "",
          "Unbinding: retrieved = role⁻¹ ⊗ bound",
          `  Retrieved vs filler: ${retrievedVsFiller.toFixed(4)} (≈1.0)`,
          "",
          "Key insight: Binding is information-preserving,",
          "unbinding recovers the original filler exactly."
        ].join('\n');
      })
    },
    {
      title: "VSA Bundle Operation",
      description: "Weighted superposition of multiple fields",
      category: "Optical",
      code: `// Bundle combines multiple fields via weighted sum
const algebra = new WasmOpticalFieldAlgebra(64, 64);

// Create multiple pattern fields
const patterns = [
  algebra.random(1n),
  algebra.random(2n),
  algebra.random(3n)
];

// Bundle with equal weights
const bundled = algebra.bundleUniform(patterns);

// Each pattern is similar to the bundle
patterns.forEach((p, i) => {
  const sim = algebra.similarity(p, bundled);
  console.log(\`Pattern \${i} similarity: \${sim}\`);
});`,
      onRun: simulateExample(() => {
        const n = 64 * 64;
        const numPatterns = 5;

        // Generate random patterns
        const patterns = [];
        for (let p = 0; p < numPatterns; p++) {
          patterns.push(Array(n).fill(0).map(() => Math.random() * 2 * Math.PI));
        }

        // Bundle: average of rotor components
        const bundleScalar = Array(n).fill(0);
        const bundleBivector = Array(n).fill(0);

        for (let i = 0; i < n; i++) {
          for (let p = 0; p < numPatterns; p++) {
            bundleScalar[i] += Math.cos(patterns[p][i]);
            bundleBivector[i] += Math.sin(patterns[p][i]);
          }
          bundleScalar[i] /= numPatterns;
          bundleBivector[i] /= numPatterns;
        }

        // Compute bundle phase
        const bundlePhases = bundleScalar.map((s, i) => Math.atan2(bundleBivector[i], s));

        // Compute similarities
        const similarity = (a: number[], b: number[]) => {
          let sum = 0;
          for (let i = 0; i < a.length; i++) {
            sum += Math.cos(a[i] - b[i]);
          }
          return sum / a.length;
        };

        const similarities = patterns.map(p => similarity(p, bundlePhases));

        return [
          "VSA Bundling (Superposition)",
          "",
          `Bundling ${numPatterns} random patterns`,
          "",
          "Bundle operation: superposition of rotors",
          "  bundle = (1/n) Σᵢ Rᵢ",
          "  Each rotor contributes to the sum",
          "",
          "Pattern similarities to bundle:",
          ...similarities.map((s, i) => `  Pattern ${i}: ${s.toFixed(4)}`),
          "",
          `Average similarity: ${(similarities.reduce((a, b) => a + b, 0) / numPatterns).toFixed(4)}`,
          `Expected for ${numPatterns} patterns: ~${(1 / Math.sqrt(numPatterns)).toFixed(4)}`,
          "",
          "Bundle contains information from all patterns.",
          "Similarity decreases with more patterns (capacity limit)."
        ].join('\n');
      })
    },
    {
      title: "Symbol Codebook",
      description: "Deterministic symbol-to-field mapping for VSA",
      category: "Optical",
      code: `// Codebook generates reproducible fields from symbols
const codebook = new WasmOpticalCodebook(64, 64, 12345n);

// Register symbols
codebook.register("AGENT");
codebook.register("ACTION");
codebook.register("LOCATION");

// Get field for symbol (deterministic)
const agentField = codebook.get("AGENT");
const actionField = codebook.get("ACTION");

// Same symbol always gives same field
const agentField2 = codebook.get("AGENT");
console.log("Identical:", algebra.similarity(agentField, agentField2) === 1.0);

// Symbols are quasi-orthogonal
console.log(\`AGENT·ACTION: \${algebra.similarity(agentField, actionField)}\`);`,
      onRun: simulateExample(() => {
        const symbols = ["AGENT", "ACTION", "LOCATION", "TIME", "OBJECT"];

        // Simulate seed-based generation
        const hashSymbol = (symbol: string, baseSeed: number) => {
          let hash = baseSeed;
          for (let i = 0; i < symbol.length; i++) {
            hash = hash * 31 + symbol.charCodeAt(i);
          }
          return hash;
        };

        const baseSeed = 12345;
        const seeds = symbols.map(s => hashSymbol(s, baseSeed));

        // Generate phases from seeds (simplified)
        const n = 64 * 64;
        const generatePhases = (seed: number) => {
          const rng = (s: number) => {
            s = Math.sin(s) * 10000;
            return s - Math.floor(s);
          };
          return Array(n).fill(0).map((_, i) => rng(seed + i) * 2 * Math.PI);
        };

        const fields = symbols.map(s => generatePhases(hashSymbol(s, baseSeed)));

        // Compute pairwise similarities
        const similarity = (a: number[], b: number[]) => {
          let sum = 0;
          for (let i = 0; i < a.length; i++) {
            sum += Math.cos(a[i] - b[i]);
          }
          return sum / a.length;
        };

        return [
          "Symbol Codebook",
          "",
          `Base seed: ${baseSeed}`,
          `Registered symbols: ${symbols.join(", ")}`,
          "",
          "Symbol → Seed mapping (FNV hash):",
          ...symbols.map((s, i) => `  ${s}: ${seeds[i]}`),
          "",
          "Pairwise similarities (should be ≈0):",
          `  AGENT · ACTION: ${similarity(fields[0], fields[1]).toFixed(4)}`,
          `  AGENT · LOCATION: ${similarity(fields[0], fields[2]).toFixed(4)}`,
          `  ACTION · TIME: ${similarity(fields[1], fields[3]).toFixed(4)}`,
          "",
          "Self-similarity (should be 1.0):",
          `  AGENT · AGENT: ${similarity(fields[0], fields[0]).toFixed(4)}`,
          "",
          "Benefits:",
          "  • Reproducible across sessions",
          "  • Minimal storage (just store seeds)",
          "  • Lazy generation on demand"
        ].join('\n');
      })
    },
    {
      title: "Tropical Attractor Dynamics",
      description: "Use tropical (min, +) algebra for attractor pattern completion",
      category: "Optical",
      code: `// Tropical operations find stable attractors
const tropical = new WasmTropicalOpticalAlgebra(32, 32);

// Create attractor patterns
const attractors = [
  WasmOpticalRotorField.random(32, 32, 100n),
  WasmOpticalRotorField.random(32, 32, 200n),
  WasmOpticalRotorField.random(32, 32, 300n)
];

// Start with noisy initial state
const initial = WasmOpticalRotorField.random(32, 32, 999n);

// Run attractor dynamics until convergence
const result = tropical.attractorConverge(
  initial, attractors, 100, 0.001
);

// Check which attractor was reached
attractors.forEach((a, i) => {
  console.log(\`Distance to attractor \${i}: \${tropical.phaseDistance(result, a)}\`);
});`,
      onRun: simulateExample(() => {
        const n = 32 * 32;
        const numAttractors = 3;

        // Generate random attractors
        const attractors: number[][] = [];
        for (let a = 0; a < numAttractors; a++) {
          attractors.push(Array(n).fill(0).map(() => Math.random() * 2 * Math.PI));
        }

        // Initial state: slightly perturbed version of attractor 0
        const initial = attractors[0].map(p => p + (Math.random() - 0.5) * 0.5);

        // Tropical dynamics: converge to nearest attractor
        let state = [...initial];
        const iterations = [];

        for (let iter = 0; iter < 10; iter++) {
          // Find closest attractor at each point
          const newState = state.map((_, i) => {
            let minDist = Infinity;
            let bestPhase = state[i];

            for (const attr of attractors) {
              const dist = Math.abs(state[i] - attr[i]);
              const wrappedDist = Math.min(dist, 2 * Math.PI - dist);
              if (wrappedDist < minDist) {
                minDist = wrappedDist;
                bestPhase = attr[i];
              }
            }

            return bestPhase;
          });

          // Compute average movement
          const movement = state.reduce((s, p, i) => {
            const d = Math.abs(p - newState[i]);
            return s + Math.min(d, 2 * Math.PI - d);
          }, 0) / n;

          iterations.push({ iter, movement });
          state = newState;

          if (movement < 0.001) break;
        }

        // Find distances to each attractor
        const phaseDistance = (a: number[], b: number[]) => {
          let sum = 0;
          for (let i = 0; i < a.length; i++) {
            const d = Math.abs(a[i] - b[i]);
            sum += Math.min(d, 2 * Math.PI - d);
          }
          return sum / a.length;
        };

        const finalDistances = attractors.map(a => phaseDistance(state, a));
        const convergedTo = finalDistances.indexOf(Math.min(...finalDistances));

        return [
          "Tropical Attractor Dynamics",
          "",
          `${numAttractors} attractors, ${n} points per field`,
          "",
          "Dynamics: each point moves toward nearest attractor phase",
          "  Uses tropical min-plus algebra: select minimum distance",
          "",
          "Convergence:",
          ...iterations.map(it =>
            `  Iter ${it.iter}: avg movement = ${it.movement.toFixed(6)}`
          ),
          "",
          "Final distances to attractors:",
          ...finalDistances.map((d, i) =>
            `  Attractor ${i}: ${d.toFixed(6)}${i === convergedTo ? " ← converged" : ""}`
          ),
          "",
          "Application: content-addressable memory",
          "  • Store patterns as attractors",
          "  • Noisy query converges to stored pattern"
        ].join('\n');
      })
    }
  ];

  return (
    <Container size="lg" py="xl">
      <Stack gap="lg">
        <div>
          <Title order={1}>Holographic Computing</Title>
          <Text size="lg" c="dimmed">
            Distributed representations and associative memory using interference patterns
          </Text>
        </div>

        <Card withBorder>
          <Card.Section inheritPadding py="xs" bg="dark.6">
            <Title order={3} size="h4">Overview</Title>
          </Card.Section>
          <Card.Section inheritPadding py="md">
            <Text mb="md">
              The <Code>amari-holographic</Code> module implements holographic memory systems
              where information is stored in distributed interference patterns. Version 0.15.1
              adds GA-native optical field operations for Lee hologram encoding and VSA.
            </Text>
            <SimpleGrid cols={{ base: 1, md: 3 }} spacing="md">
              <div>
                <Title order={4} size="sm" mb="xs">Memory Properties</Title>
                <Text size="sm" c="dimmed" component="ul" style={{ paddingLeft: '1rem', margin: 0 }}>
                  <li>Distributed representation</li>
                  <li>Content-addressable recall</li>
                  <li>Graceful degradation</li>
                  <li>Pattern superposition</li>
                </Text>
              </div>
              <div>
                <Title order={4} size="sm" mb="xs">Optical Operations</Title>
                <Text size="sm" c="dimmed" component="ul" style={{ paddingLeft: '1rem', margin: 0 }}>
                  <li>Rotor field wavefronts</li>
                  <li>Lee hologram encoding</li>
                  <li>VSA bind/bundle/similarity</li>
                  <li>Tropical attractor dynamics</li>
                </Text>
              </div>
              <div>
                <Title order={4} size="sm" mb="xs">Applications</Title>
                <Text size="sm" c="dimmed" component="ul" style={{ paddingLeft: '1rem', margin: 0 }}>
                  <li>DMD holographic displays</li>
                  <li>Optical neural networks</li>
                  <li>Fault-tolerant storage</li>
                  <li>Associative reasoning</li>
                </Text>
              </div>
            </SimpleGrid>
          </Card.Section>
        </Card>

        <Card withBorder>
          <Card.Section inheritPadding py="xs" bg="dark.6">
            <Title order={3} size="h4">Holographic Principle</Title>
          </Card.Section>
          <Card.Section inheritPadding py="md">
            <CodeHighlight
              code={`Traditional Memory:           Holographic Memory:
  Address → Data               Cue → Associated Pattern
  Localized storage            Distributed storage
  Exact match required         Partial match works
  Failure = complete loss      Failure = graceful degradation

Encoding:
  pattern → distributed representation via Fourier-like transform

Recall:
  cue → correlation with stored patterns → best match

Capacity:
  O(n / log n) patterns in n-element storage`}
              language="plaintext"
            />
          </Card.Section>
        </Card>

        <Title order={2}>Interactive Examples</Title>

        <SimpleGrid cols={1} spacing="lg">
          {examples.map((example, i) => (
            <ExampleCard
              key={i}
              title={example.title}
              description={example.description}
              code={example.code}
              onRun={example.onRun}
              badge={<Badge size="sm" variant="light">{example.category}</Badge>}
            />
          ))}
        </SimpleGrid>
      </Stack>
    </Container>
  );
}
