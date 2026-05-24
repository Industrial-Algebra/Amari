import { Title, Text, Container, Stack, Alert } from "@mantine/core";
import { ExampleCard } from "../components/ExampleCard";
import { useAmariWasm } from "../hooks/useAmariWasm";
import { LoadingState } from "../components/LoadingState";

export function GeometricAlgebra() {
  const { ready, error, amari } = useAmariWasm();

  if (!ready && !error) {
    return <LoadingState message="Loading WASM module..." />;
  }

  if (error) {
    return (
      <Container size="lg" py="xl">
        <Title order={1} mb="md">Geometric Algebra Examples</Title>
        <Alert color="red" title="Error">
          Failed to load WASM module: {error}
        </Alert>
      </Container>
    );
  }

  const runExample = (operation: () => string) => {
    return async () => {
      try {
        return operation();
      } catch (err) {
        throw new Error(`WASM operation failed: ${err}`);
      }
    };
  };

  const examples = [
    {
      title: "Basic Multivector Creation",
      description: "Create and manipulate multivectors in 3D Euclidean space (3,0,0 signature)",
      category: "Fundamentals",
      code: `// Create a 3D Euclidean multivector from coefficients
// [scalar, e1, e2, e3, e12, e13, e23, e123]
const coeffs = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
const mv = amari.WasmMultivector.fromCoefficients(coeffs);

// Access individual components
const scalar = mv.getCoefficient(0);
const e1 = mv.getCoefficient(1);
const grade1 = mv.gradeProjection(1);

console.log("Scalar component:", scalar);
console.log("e1 component:", e1);
console.log("Grade 1 coefficients:", grade1.getCoefficients());`,
      onRun: runExample(() => {
        const coeffs = new Float64Array([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
        const mv = amari!.WasmMultivector.fromCoefficients(coeffs);

        const scalar = mv.getCoefficient(0);
        const e1 = mv.getCoefficient(1);
        const grade1 = mv.gradeProjection(1);
        const grade1Coeffs = grade1.getCoefficients();

        const result = [
          `Scalar component: ${scalar}`,
          `e1 component: ${e1}`,
          `Grade 1 coefficients: [${Array.from(grade1Coeffs).join(', ')}]`
        ].join('\n');

        mv.free();
        grade1.free();

        return result;
      })
    },
    {
      title: "Geometric Product",
      description: "Compute the geometric product of two multivectors",
      category: "Operations",
      code: `// Create two basis vectors
const v1 = amari.WasmMultivector.basisVector(0); // e1
const v2 = amari.WasmMultivector.basisVector(1); // e2

// Geometric product: e1 * e2 = e1∧e2 (bivector)
const product = v1.geometricProduct(v2);

console.log("v1 coefficients:", v1.getCoefficients());
console.log("v2 coefficients:", v2.getCoefficients());
console.log("Product coefficients:", product.getCoefficients());
console.log("e12 component:", product.getCoefficient(4));`,
      onRun: runExample(() => {
        const v1 = amari!.WasmMultivector.basisVector(0);
        const v2 = amari!.WasmMultivector.basisVector(1);
        const product = v1.geometricProduct(v2);

        const v1Coeffs = v1.getCoefficients();
        const v2Coeffs = v2.getCoefficients();
        const productCoeffs = product.getCoefficients();
        const e12Component = product.getCoefficient(4);

        const result = [
          `v1 coefficients: [${Array.from(v1Coeffs).join(', ')}]`,
          `v2 coefficients: [${Array.from(v2Coeffs).join(', ')}]`,
          `Product coefficients: [${Array.from(productCoeffs).join(', ')}]`,
          `e12 component: ${e12Component}`
        ].join('\n');

        v1.free();
        v2.free();
        product.free();

        return result;
      })
    },
    {
      title: "Rotor Rotations",
      description: "Create a rotor and use it to rotate a vector",
      category: "Applications",
      code: `// Create a vector to rotate (e1)
const vector = amari.WasmMultivector.basisVector(0);

// Create bivector for rotation plane (e1∧e2)
const e1 = amari.WasmMultivector.basisVector(0);
const e2 = amari.WasmMultivector.basisVector(1);
const bivector = e1.outerProduct(e2);

// Create rotor for 90-degree rotation
const angle = Math.PI / 2; // 90 degrees
const rotor = amari.WasmRotor.fromBivector(bivector, angle);

// Apply rotation
const rotated = rotor.apply(vector);

console.log("Original vector:", vector.getCoefficients());
console.log("Rotated vector:", rotated.getCoefficients());`,
      onRun: runExample(() => {
        const vector = amari!.WasmMultivector.basisVector(0);
        const e1 = amari!.WasmMultivector.basisVector(0);
        const e2 = amari!.WasmMultivector.basisVector(1);
        const bivector = e1.outerProduct(e2);
        const angle = Math.PI / 2;
        const rotor = amari!.WasmRotor.fromBivector(bivector, angle);
        const rotated = rotor.apply(vector);

        const originalCoeffs = vector.getCoefficients();
        const rotatedCoeffs = rotated.getCoefficients();

        const result = [
          `Original vector: [${Array.from(originalCoeffs).join(', ')}]`,
          `Rotated vector: [${Array.from(rotatedCoeffs).join(', ')}]`
        ].join('\n');

        vector.free();
        e1.free();
        e2.free();
        bivector.free();
        rotor.free();
        rotated.free();

        return result;
      })
    },
    {
      title: "Inner and Outer Products",
      description: "Explore the inner product (contraction) and outer product (wedge)",
      category: "Operations",
      code: `// Create two vectors with mixed components
const coeffs1 = [0.0, 1.0, 2.0, 3.0, 0.0, 0.0, 0.0, 0.0]; // 1e1 + 2e2 + 3e3
const coeffs2 = [0.0, 4.0, 5.0, 6.0, 0.0, 0.0, 0.0, 0.0]; // 4e1 + 5e2 + 6e3

const v1 = amari.WasmMultivector.fromCoefficients(coeffs1);
const v2 = amari.WasmMultivector.fromCoefficients(coeffs2);

// Inner product (dot product for vectors)
const inner = v1.innerProduct(v2);
const innerScalar = inner.getCoefficient(0);

// Outer product (wedge product - produces bivector)
const outer = v1.outerProduct(v2);

console.log("v1:", coeffs1);
console.log("v2:", coeffs2);
console.log("Inner product (scalar):", innerScalar);
console.log("Outer product:", outer.getCoefficients());`,
      onRun: runExample(() => {
        const coeffs1 = new Float64Array([0.0, 1.0, 2.0, 3.0, 0.0, 0.0, 0.0, 0.0]);
        const coeffs2 = new Float64Array([0.0, 4.0, 5.0, 6.0, 0.0, 0.0, 0.0, 0.0]);

        const v1 = amari!.WasmMultivector.fromCoefficients(coeffs1);
        const v2 = amari!.WasmMultivector.fromCoefficients(coeffs2);
        const inner = v1.innerProduct(v2);
        const innerScalar = inner.getCoefficient(0);
        const outer = v1.outerProduct(v2);

        const outerCoeffs = outer.getCoefficients();

        const result = [
          `v1: [${Array.from(coeffs1).join(', ')}]`,
          `v2: [${Array.from(coeffs2).join(', ')}]`,
          `Inner product (scalar): ${innerScalar}`,
          `Outer product: [${Array.from(outerCoeffs).join(', ')}]`
        ].join('\n');

        v1.free();
        v2.free();
        inner.free();
        outer.free();

        return result;
      })
    },
    {
      title: "Arbitrary Signature — Generic Multivector",
      description: "Create and operate on multivectors in any Clifford algebra signature Cl(p,q,r) using the new WasmGenericMultivector (v0.23.0+)",
      category: "v0.23.0",
      code: `// Create multivectors in Cl(2,1,0) — 2+1 spacetime (e1²=+1, e2²=+1, e3²=-1)
const e1 = amari.WasmGenericMultivector.basisVector(2, 1, 0, 0);
const e2 = amari.WasmGenericMultivector.basisVector(2, 1, 0, 1);
const e3 = amari.WasmGenericMultivector.basisVector(2, 1, 0, 2);

// Spatial basis vectors square to +1
const e1sq = e1.geometricProduct(e1);
console.log("e1² =", e1sq.getCoefficient(0)); // +1

// Temporal basis vector squares to -1
const e3sq = e3.geometricProduct(e3);
console.log("e3² =", e3sq.getCoefficient(0)); // -1

// Mixed-signature bivector: e13² = +1 (positive * negative = +1)
const e13 = e1.outerProduct(e3);
const e13sq = e13.geometricProduct(e13);
console.log("e13² =", e13sq.getCoefficient(0)); // +1`,
      onRun: runExample(() => {
        const e1 = amari!.WasmGenericMultivector.basisVector(2, 1, 0, 0);
        const e2 = amari!.WasmGenericMultivector.basisVector(2, 1, 0, 1);
        const e3 = amari!.WasmGenericMultivector.basisVector(2, 1, 0, 2);

        const e1sq = e1.geometricProduct(e1);
        const e3sq = e3.geometricProduct(e3);
        const e13 = e1.outerProduct(e3);
        const e13sq = e13.geometricProduct(e13);

        const result = [
          `e1² = ${e1sq.getCoefficient(0)}`,
          `e2² = ${e2.geometricProduct(e2).getCoefficient(0)}`,
          `e3² = ${e3sq.getCoefficient(0)}`,
          `e13² = ${e13sq.getCoefficient(0)}`
        ].join('\n');

        return result;
      })
    },
    {
      title: "Minkowski Spacetime — Cl(3,1,0)",
      description: "Use the Minkowski fast-path alias for 3+1 relativistic spacetime (v0.23.0+)",
      category: "v0.23.0",
      code: `// Cl(3,1,0): e0,e1,e2 = spatial (+1), e3 = temporal (-1)
// Use the WasmMultivector310 fast-path alias
const e0 = amari.WasmMultivector310.basisVector(0);  // spatial x
const e1 = amari.WasmMultivector310.basisVector(1);  // spatial y
const e2 = amari.WasmMultivector310.basisVector(2);  // spatial z
const e3 = amari.WasmMultivector310.basisVector(3);  // time

// Spatial vectors square to +1
console.log("e0² =", e0.geometricProduct(e0).getCoefficient(0)); // +1

// Time vector squares to -1
console.log("e3² =", e3.geometricProduct(e3).getCoefficient(0)); // -1

// Spacetime bivector: e0∧e3
const e03 = e0.outerProduct(e3);
console.log("e03² =", e03.geometricProduct(e03).getCoefficient(0)); // +1`,
      onRun: runExample(() => {
        const e0 = amari!.WasmMultivector310.basisVector(0);
        const e3 = amari!.WasmMultivector310.basisVector(3);

        const e0sq = e0.geometricProduct(e0).getCoefficient(0);
        const e3sq = e3.geometricProduct(e3).getCoefficient(0);
        const e03 = e0.outerProduct(e3);
        const e03sq = e03.geometricProduct(e03).getCoefficient(0);

        const result = [
          `e0² = ${e0sq}`,
          `e3² = ${e3sq}`,
          `e03² = ${e03sq}`
        ].join('\n');

        return result;
      })
    },
    {
      title: "Quaternion Algebra — Cl(0,3,0)",
      description: "All-negative signature produces the quaternion subalgebra (v0.23.0+)",
      category: "v0.23.0",
      code: `// Cl(0,3,0): all basis vectors square to -1 (quaternion algebra)
const i = amari.WasmMultivector030.basisVector(0);
const j = amari.WasmMultivector030.basisVector(1);
const k = amari.WasmMultivector030.basisVector(2);

// i² = j² = k² = -1
console.log("i² =", i.geometricProduct(i).getCoefficient(0)); // -1
console.log("j² =", j.geometricProduct(j).getCoefficient(0)); // -1
console.log("k² =", k.geometricProduct(k).getCoefficient(0)); // -1

// i*j = k (quaternion product)
const ij = i.geometricProduct(j);
console.log("i*j = k?", ij.getCoefficient(4)); // k component`,
      onRun: runExample(() => {
        const i = amari!.WasmMultivector030.basisVector(0);
        const j = amari!.WasmMultivector030.basisVector(1);
        const k = amari!.WasmMultivector030.basisVector(2);

        const iSq = i.geometricProduct(i).getCoefficient(0);
        const jSq = j.geometricProduct(j).getCoefficient(0);
        const kSq = k.geometricProduct(k).getCoefficient(0);
        const ij = i.geometricProduct(j);
        const kComp = ij.getCoefficient(4);

        const result = [
          `i² = ${iSq}`,
          `j² = ${jSq}`,
          `k² = ${kSq}`,
          `i*j k-component = ${kComp} (expected: ±1)`
        ].join('\n');

        return result;
      })
    }
  ];

  return (
    <Container size="lg" py="xl">
      <Stack gap="lg">
        <div>
          <Title order={1} mb="sm">Geometric Algebra Examples</Title>
          <Text size="lg" c="dimmed">
            Explore multivectors, geometric products, and rotors in any Clifford algebra signature — from
            3D Euclidean through Minkowski spacetime, conformal GA, and quaternion algebra.
          </Text>
        </div>

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
      </Stack>
    </Container>
  );
}
