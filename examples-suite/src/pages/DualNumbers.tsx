import { Container, Stack, Card, Title, Text, List, SimpleGrid, Code } from "@mantine/core";
import { ExampleCard } from "../components/ExampleCard";
import { useAmariWasm } from "../hooks/useAmariWasm";

type WasmDualNumberInstance = {
  maxByPolicy(other: WasmDualNumberInstance, policy: unknown): WasmDualNumberInstance;
  getDual(): number;
};

type WasmMultiDualNumberInstance = {
  add(other: WasmMultiDualNumberInstance): WasmMultiDualNumberInstance;
  mul(other: WasmMultiDualNumberInstance): WasmMultiDualNumberInstance;
  maxByPolicy(other: WasmMultiDualNumberInstance, policy: unknown): WasmMultiDualNumberInstance;
  getReal(): number;
  getGradient(): Float64Array;
};

type WasmStaticMultiDual2Instance = {
  add(other: WasmStaticMultiDual2Instance): WasmStaticMultiDual2Instance;
  mul(other: WasmStaticMultiDual2Instance): WasmStaticMultiDual2Instance;
  maxByPolicy(other: WasmStaticMultiDual2Instance, policy: unknown): WasmStaticMultiDual2Instance;
  toMultiDual(): WasmMultiDualNumberInstance;
  getValue(): number;
  getGradient(): Float64Array;
};

type AmariWasmModule = Record<string, unknown> & {
  WasmBranchPolicy?: {
    Left: unknown;
    Right: unknown;
    Average: unknown;
  };
  WasmDualNumber?: {
    new (real: number, dual: number): WasmDualNumberInstance;
    prototype: Partial<WasmDualNumberInstance>;
  };
  WasmMultiDualNumber?: {
    variables?: (values: Float64Array) => WasmMultiDualNumberInstance[];
  };
  WasmStaticMultiDual2?: {
    variable(value: number, varIndex: number): WasmStaticMultiDual2Instance;
  };
};

function missingWasmExports(wasm: AmariWasmModule, names: string[]) {
  return names.filter((name) => wasm[name] === undefined || wasm[name] === null);
}

export function DualNumbers() {
  const { ready, error, amari } = useAmariWasm();

  // Simulate dual number operations for demonstration
  class DualNumber {
    constructor(public real: number, public dual: number) {}

    static variable(value: number): DualNumber {
      return new DualNumber(value, 1.0);
    }

    static constant(value: number): DualNumber {
      return new DualNumber(value, 0.0);
    }

    add(other: DualNumber): DualNumber {
      return new DualNumber(this.real + other.real, this.dual + other.dual);
    }

    multiply(other: DualNumber): DualNumber {
      return new DualNumber(
        this.real * other.real,
        this.real * other.dual + this.dual * other.real
      );
    }

    sin(): DualNumber {
      return new DualNumber(Math.sin(this.real), Math.cos(this.real) * this.dual);
    }

    exp(): DualNumber {
      const expVal = Math.exp(this.real);
      return new DualNumber(expVal, expVal * this.dual);
    }

    toString(): string {
      return `${this.real.toFixed(3)} + ${this.dual.toFixed(3)}ε`;
    }
  }

  const simulateExample = (operation: () => string) => {
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
      title: "Basic Dual Number Arithmetic",
      description: "Understand dual numbers and how they compute derivatives automatically",
      category: "Fundamentals",
      code: `// Dual numbers: a + bε where ε² = 0
// Real part = function value, Dual part = derivative

// Create a variable (derivative = 1) and a constant (derivative = 0)
const x = DualNumber.variable(2.0);  // 2 + 1ε
const c = DualNumber.constant(3.0);  // 3 + 0ε

console.log("x =", x.toString());
console.log("c =", c.toString());

// Addition: (a + bε) + (c + dε) = (a + c) + (b + d)ε
const sum = x.add(c);
console.log("x + c =", sum.toString());

// Multiplication: (a + bε)(c + dε) = ac + (ad + bc)ε
const product = x.multiply(c);
console.log("x * c =", product.toString());

// The dual part gives us the derivative!
console.log("d/dx(x * 3) =", product.dual, "(should be 3)");`,
      onRun: simulateExample(() => {
        const x = DualNumber.variable(2.0);
        const c = DualNumber.constant(3.0);
        const sum = x.add(c);
        const product = x.multiply(c);

        return [
          `x = ${x.toString()}`,
          `c = ${c.toString()}`,
          `x + c = ${sum.toString()}`,
          `x * c = ${product.toString()}`,
          `d/dx(x * 3) = ${product.dual} (should be 3)`
        ].join('\n');
      })
    },
    {
      title: "Branch Policies via WASM",
      description: "Use explicit 0.21.0 tie handling for non-smooth max/min points",
      category: "WASM 0.21.0",
      code: `import init, { WasmBranchPolicy, WasmDualNumber } from '@justinelliottcobb/amari-wasm';

await init();

const left = new WasmDualNumber(2.0, 1.0);
const right = new WasmDualNumber(2.0, 3.0);

const leftTie = left.maxByPolicy(right, WasmBranchPolicy.Left);
const rightTie = left.maxByPolicy(right, WasmBranchPolicy.Right);
const averageTie = left.maxByPolicy(right, WasmBranchPolicy.Average);

console.log(leftTie.getDual());    // 1
console.log(rightTie.getDual());   // 3
console.log(averageTie.getDual()); // 2`,
      onRun: wasmExample((wasm) => {
        const missing = missingWasmExports(wasm, ["WasmBranchPolicy", "WasmDualNumber"]);
        const DualNumberCtor = wasm.WasmDualNumber;
        const branchPolicy = wasm.WasmBranchPolicy;
        if (missing.length > 0 || !DualNumberCtor || !branchPolicy || typeof DualNumberCtor.prototype.maxByPolicy !== "function") {
          return "This example requires amari-wasm v0.21.0 WasmBranchPolicy and maxByPolicy bindings.";
        }

        const left = new DualNumberCtor(2.0, 1.0);
        const right = new DualNumberCtor(2.0, 3.0);
        const leftTie = left.maxByPolicy(right, branchPolicy.Left);
        const rightTie = left.maxByPolicy(right, branchPolicy.Right);
        const averageTie = left.maxByPolicy(right, branchPolicy.Average);

        return [
          "Tie at value 2.0 with derivatives 1.0 and 3.0",
          `Left policy derivative = ${leftTie.getDual()}`,
          `Right policy derivative = ${rightTie.getDual()}`,
          `Average policy derivative = ${averageTie.getDual()}`
        ].join('\n');
      })
    },
    {
      title: "Multi-Dual Seeding via WASM",
      description: "Seed a full gradient basis with the 0.21.0 multi-dual variables helper",
      category: "WASM 0.21.0",
      code: `import init, { WasmBranchPolicy, WasmMultiDualNumber } from '@justinelliottcobb/amari-wasm';

await init();

// Basis seeds for f(x, y) = x² + xy at (2, 3)
const [x, y] = WasmMultiDualNumber.variables(new Float64Array([2.0, 3.0]));
const f = x.mul(x).add(x.mul(y));

console.log(f.getReal());       // 10
console.log(f.getGradient());   // [7, 2]

const tie = x.maxByPolicy(y, WasmBranchPolicy.Average);
console.log(tie.getGradient());`,
      onRun: wasmExample((wasm) => {
        const missing = missingWasmExports(wasm, ["WasmBranchPolicy", "WasmMultiDualNumber"]);
        const MultiDualNumber = wasm.WasmMultiDualNumber;
        const branchPolicy = wasm.WasmBranchPolicy;
        if (missing.length > 0 || !MultiDualNumber || !branchPolicy || typeof MultiDualNumber.variables !== "function") {
          return "This example requires amari-wasm v0.21.0 WasmMultiDualNumber.variables bindings.";
        }

        const variables = MultiDualNumber.variables(new Float64Array([2.0, 3.0]));
        const x = variables[0];
        const y = variables[1];
        const f = x.mul(x).add(x.mul(y));
        const tie = x.maxByPolicy(y, branchPolicy.Average);

        return [
          "f(x, y) = x² + xy at (2, 3)",
          `value = ${f.getReal()}`,
          `gradient = [${Array.from(f.getGradient()).join(', ')}]`,
          `maxByPolicy gradient = [${Array.from(tie.getGradient()).join(', ')}]`
        ].join('\n');
      })
    },
    {
      title: "Static Multi-Dual Hot Loop via WASM",
      description: "Use fixed-size 0.21.0 gradient carriers for small browser optimization loops",
      category: "WASM 0.21.0",
      code: `import init, { WasmBranchPolicy, WasmStaticMultiDual2 } from '@justinelliottcobb/amari-wasm';

await init();

const x = WasmStaticMultiDual2.variable(2.0, 0);
const y = WasmStaticMultiDual2.variable(3.0, 1);

const objective = x.mul(x).add(x.mul(y));
const branch = x.maxByPolicy(y, WasmBranchPolicy.Average);
const heapBacked = objective.toMultiDual();

console.log(objective.getValue());    // 10
console.log(objective.getGradient()); // [7, 2]
console.log(branch.getGradient());
console.log(heapBacked.getGradient());`,
      onRun: wasmExample((wasm) => {
        const missing = missingWasmExports(wasm, ["WasmBranchPolicy", "WasmStaticMultiDual2"]);
        const StaticMultiDual2 = wasm.WasmStaticMultiDual2;
        const branchPolicy = wasm.WasmBranchPolicy;
        if (missing.length > 0 || !StaticMultiDual2 || !branchPolicy) {
          return "This example requires amari-wasm v0.21.0 WasmStaticMultiDual2 bindings.";
        }

        const x = StaticMultiDual2.variable(2.0, 0);
        const y = StaticMultiDual2.variable(3.0, 1);
        const objective = x.mul(x).add(x.mul(y));
        const branch = x.maxByPolicy(y, branchPolicy.Average);
        const heapBacked = objective.toMultiDual();

        return [
          "static f(x, y) = x² + xy at (2, 3)",
          `value = ${objective.getValue()}`,
          `gradient = [${Array.from(objective.getGradient()).join(', ')}]`,
          `branch gradient = [${Array.from(branch.getGradient()).join(', ')}]`,
          `converted multi-dual gradient = [${Array.from(heapBacked.getGradient()).join(', ')}]`
        ].join('\n');
      })
    },
    {
      title: "Transcendental Functions",
      description: "Automatic differentiation for sin, cos, exp, and other functions",
      category: "Functions",
      code: `// Let's compute f(x) = sin(x) and its derivative at x = π/4
const x = DualNumber.variable(Math.PI / 4);

// sin(x) with automatic differentiation
const sinX = x.sin();
console.log("x =", x.real.toFixed(3));
console.log("sin(x) =", sinX.real.toFixed(3));
console.log("d/dx sin(x) = cos(x) =", sinX.dual.toFixed(3));
console.log("Expected cos(π/4) =", Math.cos(Math.PI / 4).toFixed(3));

// Exponential function
const expX = x.exp();
console.log("\\nexp(x) =", expX.real.toFixed(3));
console.log("d/dx exp(x) = exp(x) =", expX.dual.toFixed(3));
console.log("Values match:", Math.abs(expX.real - expX.dual) < 1e-10);`,
      onRun: simulateExample(() => {
        const x = DualNumber.variable(Math.PI / 4);
        const sinX = x.sin();
        const expX = x.exp();

        return [
          `x = ${x.real.toFixed(3)}`,
          `sin(x) = ${sinX.real.toFixed(3)}`,
          `d/dx sin(x) = cos(x) = ${sinX.dual.toFixed(3)}`,
          `Expected cos(π/4) = ${Math.cos(Math.PI / 4).toFixed(3)}`,
          ``,
          `exp(x) = ${expX.real.toFixed(3)}`,
          `d/dx exp(x) = exp(x) = ${expX.dual.toFixed(3)}`,
          `Values match: ${Math.abs(expX.real - expX.dual) < 1e-10}`
        ].join('\n');
      })
    },
    {
      title: "Chain Rule Automation",
      description: "Complex function composition with automatic chain rule application",
      category: "Composition",
      code: `// Compute f(x) = sin(exp(x²)) and its derivative
// This involves multiple chain rule applications

function complexFunction(x) {
  // x²
  const xSquared = x.multiply(x);

  // exp(x²)
  const expXSquared = xSquared.exp();

  // sin(exp(x²))
  const result = expXSquared.sin();

  return result;
}

const x = DualNumber.variable(0.5);
const result = complexFunction(x);

console.log("x =", x.real);
console.log("f(x) = sin(exp(x²)) =", result.real.toFixed(6));
console.log("f'(x) =", result.dual.toFixed(6));

// Manual verification: f'(x) = cos(exp(x²)) * exp(x²) * 2x
const manual = Math.cos(Math.exp(x.real ** 2)) * Math.exp(x.real ** 2) * 2 * x.real;
console.log("Manual calculation =", manual.toFixed(6));
console.log("Match:", Math.abs(result.dual - manual) < 1e-10);`,
      onRun: simulateExample(() => {
        function complexFunction(x: DualNumber) {
          const xSquared = x.multiply(x);
          const expXSquared = xSquared.exp();
          const result = expXSquared.sin();
          return result;
        }

        const x = DualNumber.variable(0.5);
        const result = complexFunction(x);

        // Manual verification
        const manual = Math.cos(Math.exp(x.real ** 2)) * Math.exp(x.real ** 2) * 2 * x.real;

        return [
          `x = ${x.real}`,
          `f(x) = sin(exp(x²)) = ${result.real.toFixed(6)}`,
          `f'(x) = ${result.dual.toFixed(6)}`,
          `Manual calculation = ${manual.toFixed(6)}`,
          `Match: ${Math.abs(result.dual - manual) < 1e-10}`
        ].join('\n');
      })
    },
    {
      title: "Neural Network Gradient",
      description: "Compute gradients for a simple neural network layer",
      category: "Machine Learning",
      code: `// Simple linear layer: y = W*x + b
// Compute ∂loss/∂W for gradient descent

function linearLayer(x, w, b) {
  return w.multiply(x).add(b);
}

function squaredLoss(prediction, target) {
  const diff = prediction.add(target.multiply(DualNumber.constant(-1)));
  return diff.multiply(diff);
}

// Training example: x=2, target=5, initial w=1, b=0
const x = DualNumber.constant(2.0);
const target = DualNumber.constant(5.0);
const w = DualNumber.variable(1.0);  // We want gradient w.r.t. w
const b = DualNumber.constant(0.0);

// Forward pass
const prediction = linearLayer(x, w, b);
const loss = squaredLoss(prediction, target);

console.log("Input x =", x.real);
console.log("Weight w =", w.real);
console.log("Bias b =", b.real);
console.log("Prediction =", prediction.real);
console.log("Target =", target.real);
console.log("Loss =", loss.real);
console.log("∂loss/∂w =", loss.dual);

// Update rule: w_new = w - learning_rate * gradient
const learningRate = 0.1;
const newW = w.real - learningRate * loss.dual;
console.log("Updated weight =", newW.toFixed(3));`,
      onRun: simulateExample(() => {
        function linearLayer(x: DualNumber, w: DualNumber, b: DualNumber) {
          return w.multiply(x).add(b);
        }

        function squaredLoss(prediction: DualNumber, target: DualNumber) {
          const diff = prediction.add(target.multiply(DualNumber.constant(-1)));
          return diff.multiply(diff);
        }

        const x = DualNumber.constant(2.0);
        const target = DualNumber.constant(5.0);
        const w = DualNumber.variable(1.0);
        const b = DualNumber.constant(0.0);

        const prediction = linearLayer(x, w, b);
        const loss = squaredLoss(prediction, target);

        const learningRate = 0.1;
        const newW = w.real - learningRate * loss.dual;

        return [
          `Input x = ${x.real}`,
          `Weight w = ${w.real}`,
          `Bias b = ${b.real}`,
          `Prediction = ${prediction.real}`,
          `Target = ${target.real}`,
          `Loss = ${loss.real}`,
          `∂loss/∂w = ${loss.dual}`,
          `Updated weight = ${newW.toFixed(3)}`
        ].join('\n');
      })
    }
  ];

  return (
    <Container size="lg" py="xl">
      <Stack gap="lg">
        <div>
          <Title order={1}>Dual Number Automatic Differentiation</Title>
          <Text size="lg" c="dimmed">
            Explore forward-mode automatic differentiation with dual numbers for exact gradient computation.
          </Text>
        </div>

        <Card withBorder>
          <Card.Section inheritPadding py="xs" bg="dark.6">
            <Title order={3}>What are Dual Numbers?</Title>
          </Card.Section>
          <Card.Section inheritPadding py="md">
            <Text mb="md">
              Dual numbers extend real numbers with an infinitesimal unit ε where ε² = 0:
            </Text>
            <Code block mb="md">
              {`x = a + bε
where a = function value, b = derivative`}
            </Code>
            <List size="sm">
              <List.Item><Text fw={600} span>Addition</Text>: (a + bε) + (c + dε) = (a + c) + (b + d)ε</List.Item>
              <List.Item><Text fw={600} span>Multiplication</Text>: (a + bε)(c + dε) = ac + (ad + bc)ε</List.Item>
              <List.Item><Text fw={600} span>Chain Rule</Text>: Automatically applied through operations</List.Item>
              <List.Item><Text fw={600} span>No Approximation</Text>: Exact derivatives, not finite differences</List.Item>
            </List>
            <Text mt="md" size="sm" c="dimmed">
              This enables efficient forward-mode automatic differentiation without computational graphs,
              perfect for gradients in neural networks and optimization.
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
          <Card.Section inheritPadding py="xs" bg="dark.6">
            <Title order={3}>Advantages of Dual Numbers</Title>
          </Card.Section>
          <Card.Section inheritPadding py="md">
            <SimpleGrid cols={{ base: 1, sm: 2 }}>
              <div>
                <Title order={4} size="sm" mb="xs">vs. Numerical Differentiation</Title>
                <List size="sm">
                  <List.Item>Exact (no approximation error)</List.Item>
                  <List.Item>No step size tuning</List.Item>
                  <List.Item>Numerically stable</List.Item>
                </List>
              </div>
              <div>
                <Title order={4} size="sm" mb="xs">vs. Symbolic Differentiation</Title>
                <List size="sm">
                  <List.Item>No expression explosion</List.Item>
                  <List.Item>Works with any code structure</List.Item>
                  <List.Item>Efficient for many variables</List.Item>
                </List>
              </div>
            </SimpleGrid>
          </Card.Section>
        </Card>
      </Stack>
    </Container>
  );
}
