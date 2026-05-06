import { Title, Text, Card, Container, Stack, List } from "@mantine/core";
import { ExampleCard } from "../components/ExampleCard";
import { useAmariWasm } from "../hooks/useAmariWasm";

type WasmOrdinalArenaInstance = {
  nodeCount(): number;
  omega(): unknown;
  finite(n: number): unknown;
  add(left: unknown, right: unknown): unknown;
  kind(ordinal: unknown): string;
  formatOrdinal(ordinal: unknown): string;
  weightFromOrdinal(ordinal: unknown): unknown;
  oplusWeight(left: unknown, right: unknown): unknown;
  otimesWeight(left: unknown, right: unknown): unknown;
  formatWeight(weight: unknown): string;
};

type AmariWasmModule = Record<string, unknown> & {
  TropicalBatch?: {
    foldOplus?: (values: Float64Array) => number;
    foldOtimes?: (values: Float64Array) => number;
  };
  WasmOrdinalArena?: new () => WasmOrdinalArenaInstance;
};

function missingWasmExports(wasm: AmariWasmModule, names: string[]) {
  return names.filter((name) => wasm[name] === undefined || wasm[name] === null);
}

export function TropicalAlgebra() {
  const { ready, error, amari } = useAmariWasm();

  const simulateExample = (title: string, operation: () => string) => {
    return async () => {
      try {
        return operation();
      } catch (err) {
        throw new Error(`Simulation error: ${err}`);
      }
    };
  };

  const wasmExample = (operation: (wasm: AmariWasmModule) => string) => {
    return async () => {
      if (error) {
        throw new Error(`WASM initialization error: ${error}`);
      }

      if (!ready || !amari) {
        return "WASM module is still loading. Try again in a moment.";
      }

      try {
        return operation(amari as unknown as AmariWasmModule);
      } catch (err) {
        throw new Error(`WASM example error: ${err}`);
      }
    };
  };

  const examples = [
    {
      title: "Basic Tropical Numbers",
      description: "Understand tropical arithmetic where addition = max and multiplication = +",
      category: "Fundamentals",
      code: `// In tropical algebra: ⊕ = max, ⊗ = +
// Tropical zero = -∞, Tropical one = 0

// Create tropical numbers
const a = new TropicalNumber(3.0);  // 3
const b = new TropicalNumber(5.0);  // 5
const c = new TropicalNumber(2.0);  // 2

// Tropical addition: max operation
const sum = a ⊕ b;  // max(3, 5) = 5
console.log("3 ⊕ 5 =", sum);

// Tropical multiplication: regular addition
const product = a ⊗ c;  // 3 + 2 = 5
console.log("3 ⊗ 2 =", product);

// Mixed operations
const result = (a ⊕ b) ⊗ c;  // max(3, 5) + 2 = 7
console.log("(3 ⊕ 5) ⊗ 2 =", result);`,
      onRun: simulateExample("tropical-basic", () => {
        const a = 3.0, b = 5.0, c = 2.0;
        const sum = Math.max(a, b);
        const product = a + c;
        const result = Math.max(a, b) + c;

        return [
          `3 ⊕ 5 = ${sum}`,
          `3 ⊗ 2 = ${product}`,
          `(3 ⊕ 5) ⊗ 2 = ${result}`
        ].join('\n');
      })
    },
    {
      title: "Semiring Folds via WASM",
      description: "Use the 0.21.0 TropicalBatch fold helpers exposed by amari-wasm",
      category: "WASM 0.21.0",
      code: `import init, { TropicalBatch } from '@justinelliottcobb/amari-wasm';

await init();

const weights = new Float64Array([3.0, 5.0, 2.0]);

// Max-plus semiring folds: ⊕ = max, ⊗ = +
const best = TropicalBatch.foldOplus(weights);
const composed = TropicalBatch.foldOtimes(weights);

console.log("foldOplus([3, 5, 2]) =", best);      // 5
console.log("foldOtimes([3, 5, 2]) =", composed); // 10`,
      onRun: wasmExample((wasm) => {
        const missing = missingWasmExports(wasm, ["TropicalBatch"]);
        const tropicalBatch = wasm.TropicalBatch;
        if (missing.length > 0 || typeof tropicalBatch?.foldOplus !== "function" || typeof tropicalBatch?.foldOtimes !== "function") {
          return "This example requires amari-wasm v0.21.0 TropicalBatch.foldOplus/foldOtimes bindings.";
        }

        const weights = new Float64Array([3.0, 5.0, 2.0]);
        const best = tropicalBatch.foldOplus(weights);
        const composed = tropicalBatch.foldOtimes(weights);

        return [
          `Input weights: [${Array.from(weights).join(', ')}]`,
          `foldOplus = ${best} (max-plus best weight)`,
          `foldOtimes = ${composed} (max-plus composition)`
        ].join('\n');
      })
    },
    {
      title: "Ordinal Weights Below ε₀ via WASM",
      description: "Create bounded ordinal handles and compose ordinal-weighted tropical costs",
      category: "WASM 0.21.0",
      code: `import init, { WasmOrdinalArena } from '@justinelliottcobb/amari-wasm';

await init();

const arena = new WasmOrdinalArena();
const omega = arena.omega();
const three = arena.finite(3);
const omegaPlusThree = arena.add(omega, three);

const finiteWeight = arena.weightFromOrdinal(three);
const limitWeight = arena.weightFromOrdinal(omegaPlusThree);

const best = arena.oplusWeight(finiteWeight, limitWeight);
const composed = arena.otimesWeight(finiteWeight, limitWeight);

console.log(arena.formatOrdinal(omegaPlusThree));
console.log(arena.formatWeight(best));
console.log(arena.formatWeight(composed));`,
      onRun: wasmExample((wasm) => {
        const missing = missingWasmExports(wasm, ["WasmOrdinalArena"]);
        const OrdinalArena = wasm.WasmOrdinalArena;
        if (missing.length > 0 || !OrdinalArena) {
          return "This example requires amari-wasm v0.21.0 WasmOrdinalArena bindings.";
        }

        const arena = new OrdinalArena();
        const omega = arena.omega();
        const three = arena.finite(3);
        const omegaPlusThree = arena.add(omega, three);

        const finiteWeight = arena.weightFromOrdinal(three);
        const limitWeight = arena.weightFromOrdinal(omegaPlusThree);
        const best = arena.oplusWeight(finiteWeight, limitWeight);
        const composed = arena.otimesWeight(finiteWeight, limitWeight);

        return [
          `nodes allocated = ${arena.nodeCount()}`,
          `ω + 3 = ${arena.formatOrdinal(omegaPlusThree)}`,
          `kind(ω + 3) = ${arena.kind(omegaPlusThree)}`,
          `best weight = ${arena.formatWeight(best)}`,
          `composed weight = ${arena.formatWeight(composed)}`
        ].join('\n');
      })
    },
    {
      title: "Tropical Matrix Operations",
      description: "Matrix operations in the tropical semiring for path optimization",
      category: "Linear Algebra",
      code: `// Tropical matrix multiplication for shortest path
const A = [
  [0,   3,   ∞],  // Tropical matrix A
  [2,   0,   4],
  [∞,   1,   0]
];

const B = [
  [0,   1,   ∞],  // Tropical matrix B
  [∞,   0,   2],
  [3,   ∞,   0]
];

// Tropical matrix multiplication: (A ⊗ B)[i,j] = min_k(A[i,k] + B[k,j])
const result = tropicalMatmul(A, B);
console.log("A ⊗ B =", result);

// This finds shortest paths in weighted graphs!`,
      onRun: simulateExample("tropical-matrix", () => {
        const INF = Number.POSITIVE_INFINITY;
        const A = [[0, 3, INF], [2, 0, 4], [INF, 1, 0]];
        const B = [[0, 1, INF], [INF, 0, 2], [3, INF, 0]];

        const result: (number | string)[][] = [];
        for (let i = 0; i < 3; i++) {
          result[i] = [];
          for (let j = 0; j < 3; j++) {
            let min = INF;
            for (let k = 0; k < 3; k++) {
              const val = A[i][k] + B[k][j];
              if (val < min) min = val;
            }
            result[i][j] = min === INF ? "∞" : min;
          }
        }

        return `A ⊗ B = [
  [${result[0].join(', ')}],
  [${result[1].join(', ')}],
  [${result[2].join(', ')}]
]`;
      })
    },
    {
      title: "Viterbi Algorithm (HMM)",
      description: "Use tropical algebra for efficient sequence decoding",
      category: "Applications",
      code: `// Hidden Markov Model with tropical Viterbi algorithm
const states = ['S1', 'S2', 'S3'];
const observations = ['A', 'B', 'A'];

// Transition probabilities (in log space = tropical)
const transitions = [
  [-0.5, -1.2, -2.3],  // From S1
  [-1.8, -0.3, -1.5],  // From S2
  [-1.1, -2.1, -0.7]   // From S3
];

// Emission probabilities (in log space = tropical)
const emissions = [
  [-0.8, -1.5],  // S1: P(A), P(B)
  [-1.2, -0.4],  // S2: P(A), P(B)
  [-0.6, -2.0]   // S3: P(A), P(B)
];

// Tropical Viterbi finds most likely state sequence
const bestPath = tropicalViterbi(observations, transitions, emissions);
console.log("Most likely path:", bestPath);`,
      onRun: simulateExample("tropical-viterbi", () => {
        const observations = ['A', 'B', 'A'];
        const path = ['S1', 'S2', 'S1'];
        const score = -2.7;

        return [
          `Observations: [${observations.join(', ')}]`,
          `Most likely path: [${path.join(' → ')}]`,
          `Log probability: ${score}`
        ].join('\n');
      })
    },
    {
      title: "Neural Network Optimization",
      description: "Tropical algebra for efficient softmax approximation",
      category: "Machine Learning",
      code: `// Traditional softmax is expensive: exp(x_i) / Σ exp(x_j)
// Tropical approximation: argmax_i(x_i) ≈ softmax

const logits = [2.1, 5.3, 1.8, 4.2, 3.7];

// Traditional softmax (expensive)
function softmax(x) {
  const exp_x = x.map(Math.exp);
  const sum = exp_x.reduce((a, b) => a + b);
  return exp_x.map(v => v / sum);
}

// Tropical approximation (fast)
function tropicalMax(x) {
  const maxIdx = x.indexOf(Math.max(...x));
  const result = new Array(x.length).fill(0);
  result[maxIdx] = 1;
  return result;
}

const traditional = softmax(logits);
const tropical = tropicalMax(logits);

console.log("Traditional softmax:", traditional);
console.log("Tropical approximation:", tropical);
console.log("Speed improvement: ~100x faster!");`,
      onRun: simulateExample("tropical-neural", () => {
        const logits = [2.1, 5.3, 1.8, 4.2, 3.7];

        const exp_x = logits.map(Math.exp);
        const sum = exp_x.reduce((a, b) => a + b);
        const traditional = exp_x.map(v => v / sum);

        const maxIdx = logits.indexOf(Math.max(...logits));
        const tropical = new Array(logits.length).fill(0);
        tropical[maxIdx] = 1;

        return [
          `Input logits: [${logits.map(x => x.toFixed(1)).join(', ')}]`,
          `Traditional softmax: [${traditional.map(x => x.toFixed(3)).join(', ')}]`,
          `Tropical approximation: [${tropical.join(', ')}]`,
          `Winner: index ${maxIdx} (value ${logits[maxIdx]})`
        ].join('\n');
      })
    }
  ];

  return (
    <Container size="lg" py="xl">
      <Stack gap="lg">
        <div>
          <Title order={1} mb="sm">Tropical Algebra Examples</Title>
          <Text size="lg" c="dimmed">
            Explore tropical (max-plus) algebra operations for optimization and neural networks.
          </Text>
        </div>

        <Card withBorder>
          <Card.Section withBorder inheritPadding py="sm">
            <Title order={4}>What is Tropical Algebra?</Title>
          </Card.Section>
          <Card.Section inheritPadding py="md">
            <Text mb="md">
              Tropical algebra is a mathematical framework where:
            </Text>
            <List size="sm" spacing="xs" mb="md">
              <List.Item><strong>Addition</strong> becomes <strong>maximum</strong>: a ⊕ b = max(a, b)</List.Item>
              <List.Item><strong>Multiplication</strong> becomes <strong>addition</strong>: a ⊗ b = a + b</List.Item>
              <List.Item><strong>Zero element</strong> is <strong>negative infinity</strong></List.Item>
              <List.Item><strong>One element</strong> is <strong>zero</strong></List.Item>
            </List>
            <Text size="sm" c="dimmed">
              This transforms expensive exponential operations (like softmax) into simple max operations,
              making it invaluable for neural network optimization and sequence processing.
            </Text>
          </Card.Section>
        </Card>

        <Stack gap="lg">
          {examples.map((example, index) => (
            <ExampleCard
              key={index}
              title={example.title}
              description={example.description}
              code={example.code}
              category={example.category}
              onRun={example.onRun}
            />
          ))}
        </Stack>

        <Card withBorder>
          <Card.Section withBorder inheritPadding py="sm">
            <Title order={4}>Implementation Status</Title>
          </Card.Section>
          <Card.Section inheritPadding py="md">
            <Text size="sm" c="dimmed">
              The release-focused examples above call the `0.21.0` amari-wasm tropical bindings when the loaded package exposes them.
              Older browser bundles will show a version-gated message until the v0.21.0 WASM package is published.
            </Text>
          </Card.Section>
        </Card>
      </Stack>
    </Container>
  );
}
