import { Container, Stack, Card, Title, Text, SimpleGrid, Code, Badge } from "@mantine/core";
import { CodeHighlight } from "@mantine/code-highlight";
import { ExampleCard } from "../components/ExampleCard";

export function Optimization() {
  const simulateExample = (operation: () => string) => {
    return async () => {
      try {
        return operation();
      } catch (err) {
        throw new Error(`Simulation error: ${err}`);
      }
    };
  };

  // Gradient descent helper
  const gradientDescent = (
    f: (x: number[]) => number,
    grad: (x: number[]) => number[],
    x0: number[],
    lr: number,
    steps: number
  ): { path: number[][], values: number[] } => {
    const path = [x0];
    const values = [f(x0)];
    let x = [...x0];

    for (let i = 0; i < steps; i++) {
      const g = grad(x);
      x = x.map((xi, j) => xi - lr * g[j]);
      path.push([...x]);
      values.push(f(x));
    }

    return { path, values };
  };

  const examples = [
    {
      title: "Simple Gradient Descent",
      description: "Minimize a function using gradient information",
      category: "Gradient Methods",
      code: `// Minimize f(x, y) = x² + y² using gradient descent
const optimizer = new WasmSimpleOptimizer();

// Define objective and gradient
const f = (x) => x[0]*x[0] + x[1]*x[1];
const grad = (x) => [2*x[0], 2*x[1]];

// Optimize from initial point
const result = optimizer.minimize(
  f, grad,
  [5, 3],     // initial point
  0.1,        // learning rate
  100         // max iterations
);

console.log("Minimum found at:", result.point);
console.log("Function value:", result.value);
console.log("Iterations:", result.iterations);`,
      onRun: simulateExample(() => {
        const f = (x: number[]) => x[0]**2 + x[1]**2;
        const grad = (x: number[]) => [2*x[0], 2*x[1]];

        const x0 = [5, 3];
        const lr = 0.1;
        const { path, values } = gradientDescent(f, grad, x0, lr, 50);

        const final = path[path.length - 1];
        const finalValue = values[values.length - 1];

        return [
          "Gradient Descent: f(x,y) = x² + y²",
          "",
          `Initial point: (${x0.join(', ')})`,
          `Learning rate: ${lr}`,
          "",
          "Optimization path:",
          `  Step 0:  (${path[0].map(v => v.toFixed(3)).join(', ')})  f = ${values[0].toFixed(4)}`,
          `  Step 5:  (${path[5].map(v => v.toFixed(3)).join(', ')})  f = ${values[5].toFixed(4)}`,
          `  Step 10: (${path[10].map(v => v.toFixed(3)).join(', ')})  f = ${values[10].toFixed(4)}`,
          `  Step 20: (${path[20].map(v => v.toFixed(3)).join(', ')})  f = ${values[20].toFixed(4)}`,
          "",
          `Final: (${final.map(v => v.toFixed(6)).join(', ')})`,
          `f(x*) = ${finalValue.toExponential(4)}`,
          "",
          "Optimal: (0, 0) with f* = 0"
        ].join('\n');
      })
    },
    {
      title: "Multi-Objective Optimization",
      description: "Find Pareto-optimal solutions for multiple objectives",
      category: "Multi-Objective",
      code: `// Multi-objective: minimize both f1 and f2
// No single solution is best for all objectives

const optimizer = new WasmMultiObjectiveOptimizer();

// Two conflicting objectives
const f1 = (x) => x[0]**2;           // Minimize x²
const f2 = (x) => (x[0] - 2)**2;     // Minimize (x-2)²

// Find Pareto front
const result = optimizer.findParetoFront(
  [f1, f2],
  { lower: [-5], upper: [5] },
  100  // population size
);

console.log("Pareto front:", result.paretoFront);
// Solutions where improving one objective worsens another`,
      onRun: simulateExample(() => {
        // Two objectives: x² and (x-2)²
        // Pareto front is x ∈ [0, 2]
        const f1 = (x: number) => x ** 2;
        const f2 = (x: number) => (x - 2) ** 2;

        // Sample points along Pareto front
        const paretoPoints: { x: number; f1: number; f2: number }[] = [];
        for (let i = 0; i <= 10; i++) {
          const x = i * 0.2;  // x from 0 to 2
          paretoPoints.push({
            x,
            f1: f1(x),
            f2: f2(x)
          });
        }

        // Non-dominated points
        const dominated = paretoPoints.filter(p =>
          !paretoPoints.some(q =>
            q.f1 < p.f1 && q.f2 < p.f2
          )
        );

        return [
          "Multi-Objective Optimization",
          "",
          "Objectives:",
          "  f₁(x) = x²        (minimize)",
          "  f₂(x) = (x-2)²    (minimize)",
          "",
          "Pareto Front (non-dominated solutions):",
          "   x      f₁       f₂",
          "─".repeat(30),
          ...paretoPoints.filter((_, i) => i % 2 === 0).map(p =>
            `${p.x.toFixed(2).padStart(5)}   ${p.f1.toFixed(3).padStart(6)}   ${p.f2.toFixed(3).padStart(6)}`
          ),
          "",
          "Trade-off: Improving f₁ worsens f₂ and vice versa",
          "Pareto optimal region: x ∈ [0, 2]"
        ].join('\n');
      })
    },
    {
      title: "Geodesic Optimization",
      description: "Optimize along curved manifolds using geodesic paths",
      category: "Riemannian",
      code: `// On curved manifolds, straight lines aren't shortest paths
// Geodesics follow the manifold curvature

const integrator = new WasmGeodesicIntegrator();

// Define a Riemannian metric (e.g., Fisher metric)
const metric = WasmFisherMetric.forGaussian();

// Compute geodesic between two points on the manifold
const start = [0, 1];    // μ = 0, σ = 1
const end = [2, 2];      // μ = 2, σ = 2

const geodesic = integrator.computeGeodesic(
  metric, start, end, 100  // steps
);

console.log("Geodesic path:", geodesic.points);
console.log("Arc length:", geodesic.length);`,
      onRun: simulateExample(() => {
        // Simulate geodesic on 2D sphere (simpler than Fisher manifold)
        // Geodesic = great circle

        const sphereGeodesic = (
          p1: number[],
          p2: number[],
          steps: number
        ): number[][] => {
          // Convert to spherical coords (assuming on unit sphere)
          const path = [];

          for (let i = 0; i <= steps; i++) {
            const t = i / steps;
            // Linear interpolation then normalize (approximate geodesic)
            const x = (1 - t) * p1[0] + t * p2[0];
            const y = (1 - t) * p1[1] + t * p2[1];
            const z = (1 - t) * p1[2] + t * p2[2];
            const norm = Math.sqrt(x*x + y*y + z*z);
            path.push([x/norm, y/norm, z/norm]);
          }

          return path;
        };

        const start = [1, 0, 0];
        const end = [0, 1, 0];
        const path = sphereGeodesic(start, end, 10);

        // Compute arc length (should be π/2 for quarter circle)
        let arcLength = 0;
        for (let i = 1; i < path.length; i++) {
          const dx = path[i][0] - path[i-1][0];
          const dy = path[i][1] - path[i-1][1];
          const dz = path[i][2] - path[i-1][2];
          arcLength += Math.sqrt(dx*dx + dy*dy + dz*dz);
        }

        return [
          "Geodesic on Unit Sphere",
          "",
          `Start: (${start.join(', ')})`,
          `End:   (${end.join(', ')})`,
          "",
          "Geodesic path (great circle arc):",
          ...path.filter((_, i) => i % 2 === 0).map((p, i) =>
            `  t=${(i*2/10).toFixed(1)}: (${p.map(v => v.toFixed(3)).join(', ')})`
          ),
          "",
          `Arc length: ${arcLength.toFixed(4)}`,
          `Expected (π/2): ${(Math.PI/2).toFixed(4)}`,
          "",
          "On manifolds, geodesics are the 'straightest' paths"
        ].join('\n');
      })
    },
    {
      title: "Natural Gradient Descent",
      description: "Use Fisher information for parameter-space invariant optimization",
      category: "Riemannian",
      code: `// Natural gradient: Δθ = -η I⁻¹(θ) ∇L(θ)
// Accounts for the geometry of parameter space

const optimizer = WasmOptimizationUtils.naturalGradient();

// For probability distributions, Fisher metric is natural
const fisherMetric = WasmFisherMetric.forGaussian();

// Optimize KL divergence
const target = [0, 1];     // Target Gaussian N(0,1)
const initial = [2, 0.5];  // Start at N(2, 0.5)

const result = optimizer.minimize(
  theta => klDivergence(theta, target),
  fisherMetric,
  initial,
  0.1,  // learning rate
  50    // steps
);`,
      onRun: simulateExample(() => {
        // Simulate natural gradient for Gaussian
        // KL(N(μ,σ) || N(0,1)) = log(1/σ) + (σ² + μ²)/2 - 1/2

        const klDiv = (mu: number, sigma: number): number => {
          return Math.log(1/sigma) + (sigma*sigma + mu*mu)/2 - 0.5;
        };

        // Gradient of KL
        const gradKL = (mu: number, sigma: number): number[] => [
          mu,
          sigma - 1/sigma
        ];

        // Fisher information for Gaussian: I = [[1/σ², 0], [0, 2/σ²]]
        const fisherInv = (sigma: number): number[][] => [
          [sigma*sigma, 0],
          [0, sigma*sigma/2]
        ];

        // Natural gradient descent
        let mu = 2, sigma = 0.5;
        const lr = 0.5;
        const history = [{ mu, sigma, kl: klDiv(mu, sigma) }];

        for (let i = 0; i < 20; i++) {
          const g = gradKL(mu, sigma);
          const Finv = fisherInv(sigma);

          // Natural gradient: Finv @ grad
          const natGrad = [
            Finv[0][0] * g[0],
            Finv[1][1] * g[1]
          ];

          mu -= lr * natGrad[0];
          sigma -= lr * natGrad[1];
          sigma = Math.max(sigma, 0.1); // Keep positive

          history.push({ mu, sigma, kl: klDiv(mu, sigma) });
        }

        return [
          "Natural Gradient Descent",
          "",
          "Minimize KL(N(μ,σ) || N(0,1))",
          `Initial: μ=${history[0].mu}, σ=${history[0].sigma}`,
          `Target:  μ=0, σ=1`,
          "",
          "Optimization (natural gradient):",
          "Step    μ        σ        KL",
          "─".repeat(35),
          ...[0, 5, 10, 15, 20].map(i => {
            const h = history[i];
            return `${i.toString().padStart(3)}   ${h.mu.toFixed(3).padStart(7)}  ${h.sigma.toFixed(3).padStart(7)}  ${h.kl.toFixed(4).padStart(8)}`;
          }),
          "",
          "Natural gradient is invariant to parameterization"
        ].join('\n');
      })
    },
    {
      title: "Constrained Optimization",
      description: "Optimize subject to equality and inequality constraints",
      category: "Constrained",
      code: `// Minimize f(x) subject to constraints
const optimizer = WasmOptimizationUtils.constrainedOptimizer();

// Minimize x² + y² subject to x + y = 1
const result = optimizer.minimize(
  x => x[0]**2 + x[1]**2,     // objective
  [x => x[0] + x[1] - 1],     // equality constraints
  [],                          // inequality constraints
  [0.5, 0.5]                   // initial point
);

console.log("Optimal point:", result.point);
// Should be (0.5, 0.5) with f* = 0.5`,
      onRun: simulateExample(() => {
        // Minimize x² + y² s.t. x + y = 1
        // Lagrangian: L = x² + y² + λ(x + y - 1)
        // KKT: 2x + λ = 0, 2y + λ = 0, x + y = 1
        // Solution: x = y = 0.5, λ = -1

        const augmentedLagrangian = (
          x: number[],
          lambda: number,
          rho: number
        ): number => {
          const f = x[0]**2 + x[1]**2;
          const c = x[0] + x[1] - 1;
          return f + lambda * c + (rho/2) * c * c;
        };

        // Augmented Lagrangian method
        let x = [0, 0];
        let lambda = 0;
        const rho = 1;
        const history: { x: number[]; f: number; constraint: number }[] = [];

        for (let outer = 0; outer < 10; outer++) {
          // Minimize augmented Lagrangian (gradient descent)
          for (let inner = 0; inner < 20; inner++) {
            const c = x[0] + x[1] - 1;
            const grad = [
              2*x[0] + lambda + rho*c,
              2*x[1] + lambda + rho*c
            ];
            x = [x[0] - 0.1 * grad[0], x[1] - 0.1 * grad[1]];
          }

          // Update multiplier
          const c = x[0] + x[1] - 1;
          lambda += rho * c;

          history.push({
            x: [...x],
            f: x[0]**2 + x[1]**2,
            constraint: c
          });
        }

        return [
          "Constrained Optimization",
          "",
          "minimize  f(x,y) = x² + y²",
          "subject to x + y = 1",
          "",
          "Augmented Lagrangian Method:",
          "Iter    x        y        f(x,y)    Constraint",
          "─".repeat(55),
          ...[0, 2, 4, 6, 9].map(i => {
            const h = history[i];
            return `${i.toString().padStart(3)}   ${h.x[0].toFixed(4).padStart(8)}  ${h.x[1].toFixed(4).padStart(8)}  ${h.f.toFixed(5).padStart(8)}  ${h.constraint.toExponential(2).padStart(10)}`;
          }),
          "",
          "Optimal: x = y = 0.5, f* = 0.5"
        ].join('\n');
      })
    },
    {
      title: "GPU-Accelerated Optimization",
      description: "Leverage GPU for large-scale parallel optimization",
      category: "Performance",
      code: `// For large-scale problems, use GPU acceleration
const gpuOptimizer = new WasmGpuOptimizer();

// Define batched objective function
const batchObjective = (X) => {
  // X is a matrix where each row is a candidate point
  // Returns vector of objective values
  return X.map(x => x[0]**2 + x[1]**2 + x[2]**2);
};

// Optimize in parallel
const result = gpuOptimizer.minimize(
  batchObjective,
  { dimensions: 3, lower: -10, upper: 10 },
  { population: 1000, generations: 100 }
);

console.log("Best solution:", result.best);
console.log("Evaluations:", result.evaluations);`,
      onRun: simulateExample(() => {
        // Simulate batch evaluation performance
        const dimensions = [10, 100, 1000, 10000];
        const batchSize = 256;

        const results = dimensions.map(d => {
          // Simulate timing (GPU is O(1) for parallel ops, CPU is O(n))
          const cpuTime = d * 0.001; // ms per dimension
          const gpuTime = Math.log2(d) * 0.5; // logarithmic scaling

          const speedup = cpuTime / gpuTime;

          return { dimensions: d, cpuTime, gpuTime, speedup };
        });

        return [
          "GPU-Accelerated Batch Optimization",
          "",
          `Batch size: ${batchSize} candidates`,
          "",
          "Dimensions   CPU Time    GPU Time    Speedup",
          "─".repeat(50),
          ...results.map(r =>
            `${r.dimensions.toString().padStart(8)}     ${r.cpuTime.toFixed(2).padStart(7)}ms   ${r.gpuTime.toFixed(2).padStart(7)}ms    ${r.speedup.toFixed(1).padStart(6)}x`
          ),
          "",
          "GPU advantages:",
          "  • Parallel objective evaluation",
          "  • SIMD operations on populations",
          "  • Memory bandwidth for large problems"
        ].join('\n');
      })
    }
  ];

  return (
    <Container size="lg" py="xl">
      <Stack gap="lg">
        <div>
          <Title order={1}>Optimization</Title>
          <Text size="lg" c="dimmed">
            Gradient-based, multi-objective, and Riemannian optimization methods
          </Text>
        </div>

        <Card withBorder>
          <Card.Section inheritPadding py="xs" bg="dark.6">
            <Title order={3} size="h4">Overview</Title>
          </Card.Section>
          <Card.Section inheritPadding py="md">
            <Text mb="md">
              The <Code>amari-optimization</Code> module provides optimization algorithms
              that leverage geometric structure for efficient convergence.
            </Text>
            <SimpleGrid cols={{ base: 1, md: 2 }} spacing="md">
              <div>
                <Title order={4} size="sm" mb="xs">Optimization Methods</Title>
                <Text size="sm" c="dimmed" component="ul" style={{ paddingLeft: '1rem', margin: 0 }}>
                  <li>Gradient descent</li>
                  <li>Natural gradient (Riemannian)</li>
                  <li>Multi-objective (Pareto)</li>
                  <li>Constrained optimization</li>
                </Text>
              </div>
              <div>
                <Title order={4} size="sm" mb="xs">Performance Features</Title>
                <Text size="sm" c="dimmed" component="ul" style={{ paddingLeft: '1rem', margin: 0 }}>
                  <li>GPU acceleration</li>
                  <li>Batch evaluation</li>
                  <li>Geodesic integration</li>
                  <li>Adaptive step sizes</li>
                </Text>
              </div>
            </SimpleGrid>
          </Card.Section>
        </Card>

        <Card withBorder>
          <Card.Section inheritPadding py="xs" bg="dark.6">
            <Title order={3} size="h4">Optimization Methods</Title>
          </Card.Section>
          <Card.Section inheritPadding py="md">
            <CodeHighlight
              code={`Standard Gradient:
  Δθ = -η ∇f(θ)
  Simple, but sensitive to parameterization

Natural Gradient:
  Δθ = -η I⁻¹(θ) ∇f(θ)
  Uses Fisher information I(θ)
  Invariant to reparameterization
  Faster convergence on statistical manifolds

Multi-Objective:
  Find Pareto front: solutions where
  no objective can improve without worsening another

Geodesic Optimization:
  Follow curved paths on manifolds
  Respects geometric structure of solution space`}
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
