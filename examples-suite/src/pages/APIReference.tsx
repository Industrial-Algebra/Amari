import { Container, Stack, Card, Title, Text, Badge, SimpleGrid, Accordion, Group, Box, Tabs } from "@mantine/core";
import { CodeHighlight } from "@mantine/code-highlight";
import { useState } from "react";

interface ApiMethod {
  name: string;
  signature: string;
  description: string;
  example?: string;
  parameters?: { name: string; type: string; description: string; }[];
  returns?: string;
  isStatic?: boolean;
}

interface ApiClass {
  name: string;
  description: string;
  methods: ApiMethod[];
}

interface ApiSection {
  id: string;
  title: string;
  description: string;
  icon: string;
  classes: ApiClass[];
  functions?: ApiMethod[];
}

const apiSections: ApiSection[] = [
  {
    id: "geometric",
    title: "Geometric Algebra",
    description: "Multivector operations, geometric products, rotors, and transformations in Cl(3,0,0)",
    icon: "△",
    classes: [
      {
        name: "WasmMultivector",
        description: "Core multivector type representing elements of Cl(3,0,0) with 8 basis components: 1, e₁, e₂, e₃, e₁₂, e₁₃, e₂₃, e₁₂₃",
        methods: [
          {
            name: "constructor",
            signature: "new WasmMultivector()",
            description: "Create a new zero multivector",
            returns: "WasmMultivector",
            example: `const mv = new WasmMultivector();
console.log(mv.getCoefficients()); // [0, 0, 0, 0, 0, 0, 0, 0]`
          },
          {
            name: "scalar",
            signature: "static scalar(value: number): WasmMultivector",
            description: "Create a scalar multivector (grade 0)",
            isStatic: true,
            parameters: [{ name: "value", type: "number", description: "Scalar value" }],
            returns: "WasmMultivector with only scalar component",
            example: `const s = WasmMultivector.scalar(3.14);
console.log(s.getCoefficient(0)); // 3.14`
          },
          {
            name: "basisVector",
            signature: "static basisVector(index: number): WasmMultivector",
            description: "Create a basis vector e₁, e₂, or e₃ (indices 0, 1, 2)",
            isStatic: true,
            parameters: [{ name: "index", type: "number", description: "Basis index (0=e₁, 1=e₂, 2=e₃)" }],
            returns: "WasmMultivector representing the basis vector",
            example: `const e1 = WasmMultivector.basisVector(0);
const e2 = WasmMultivector.basisVector(1);
console.log(e1.getCoefficients()); // [0, 1, 0, 0, 0, 0, 0, 0]`
          },
          {
            name: "fromCoefficients",
            signature: "static fromCoefficients(coefficients: Float64Array): WasmMultivector",
            description: "Create multivector from 8 coefficients [scalar, e₁, e₂, e₃, e₁₂, e₁₃, e₂₃, e₁₂₃]",
            isStatic: true,
            parameters: [{ name: "coefficients", type: "Float64Array", description: "8 basis coefficients" }],
            returns: "WasmMultivector",
            example: `const mv = WasmMultivector.fromCoefficients(
  new Float64Array([1, 2, 3, 4, 0, 0, 0, 0])
); // 1 + 2e₁ + 3e₂ + 4e₃`
          },
          {
            name: "geometricProduct",
            signature: "geometricProduct(other: WasmMultivector): WasmMultivector",
            description: "Compute the geometric product AB = A·B + A∧B",
            parameters: [{ name: "other", type: "WasmMultivector", description: "Right operand" }],
            returns: "WasmMultivector result",
            example: `const e1 = WasmMultivector.basisVector(0);
const e2 = WasmMultivector.basisVector(1);
const e12 = e1.geometricProduct(e2);
console.log(e12.getCoefficient(4)); // 1.0 (e₁₂ component)`
          },
          {
            name: "innerProduct",
            signature: "innerProduct(other: WasmMultivector): WasmMultivector",
            description: "Compute the inner product (contraction) A·B",
            parameters: [{ name: "other", type: "WasmMultivector", description: "Right operand" }],
            returns: "WasmMultivector result",
            example: `const v1 = WasmMultivector.fromCoefficients(new Float64Array([0,1,2,3,0,0,0,0]));
const v2 = WasmMultivector.fromCoefficients(new Float64Array([0,4,5,6,0,0,0,0]));
const dot = v1.innerProduct(v2);
console.log(dot.getCoefficient(0)); // 1*4 + 2*5 + 3*6 = 32`
          },
          {
            name: "outerProduct",
            signature: "outerProduct(other: WasmMultivector): WasmMultivector",
            description: "Compute the outer (wedge) product A∧B",
            parameters: [{ name: "other", type: "WasmMultivector", description: "Right operand" }],
            returns: "WasmMultivector result",
            example: `const e1 = WasmMultivector.basisVector(0);
const e2 = WasmMultivector.basisVector(1);
const e12 = e1.outerProduct(e2); // e₁∧e₂ = e₁₂`
          },
          {
            name: "add",
            signature: "add(other: WasmMultivector): WasmMultivector",
            description: "Add two multivectors",
            parameters: [{ name: "other", type: "WasmMultivector", description: "Addend" }],
            returns: "WasmMultivector sum"
          },
          {
            name: "sub",
            signature: "sub(other: WasmMultivector): WasmMultivector",
            description: "Subtract two multivectors",
            parameters: [{ name: "other", type: "WasmMultivector", description: "Subtrahend" }],
            returns: "WasmMultivector difference"
          },
          {
            name: "scale",
            signature: "scale(scalar: number): WasmMultivector",
            description: "Scale multivector by a scalar",
            parameters: [{ name: "scalar", type: "number", description: "Scale factor" }],
            returns: "Scaled WasmMultivector"
          },
          {
            name: "reverse",
            signature: "reverse(): WasmMultivector",
            description: "Compute the reverse (reversion) Ã - reverses order of basis vectors in each term",
            returns: "Reversed WasmMultivector"
          },
          {
            name: "inverse",
            signature: "inverse(): WasmMultivector",
            description: "Compute the multiplicative inverse A⁻¹",
            returns: "Inverse WasmMultivector"
          },
          {
            name: "exp",
            signature: "exp(): WasmMultivector",
            description: "Compute exponential exp(A) - especially useful for bivectors to create rotors",
            returns: "WasmMultivector",
            example: `// Create a rotor for 90° rotation in the e₁e₂ plane
const bivector = WasmMultivector.fromCoefficients(
  new Float64Array([0, 0, 0, 0, Math.PI/4, 0, 0, 0])
);
const rotor = bivector.exp();`
          },
          {
            name: "magnitude",
            signature: "magnitude(): number",
            description: "Compute the magnitude |A| = √(A·Ã)",
            returns: "Scalar magnitude"
          },
          {
            name: "normalize",
            signature: "normalize(): WasmMultivector",
            description: "Normalize to unit magnitude",
            returns: "Normalized WasmMultivector"
          },
          {
            name: "gradeProjection",
            signature: "gradeProjection(grade: number): WasmMultivector",
            description: "Project onto a specific grade (0=scalar, 1=vector, 2=bivector, 3=pseudoscalar)",
            parameters: [{ name: "grade", type: "number", description: "Grade to project onto (0-3)" }],
            returns: "Grade-projected WasmMultivector"
          },
          {
            name: "getCoefficients",
            signature: "getCoefficients(): Float64Array",
            description: "Get all 8 coefficients as a typed array",
            returns: "Float64Array of 8 coefficients"
          },
          {
            name: "getCoefficient",
            signature: "getCoefficient(index: number): number",
            description: "Get a specific coefficient",
            parameters: [{ name: "index", type: "number", description: "Coefficient index (0-7)" }],
            returns: "Coefficient value"
          }
        ]
      },
      {
        name: "WasmRotor",
        description: "Rotor for efficient 3D rotations using geometric algebra. Rotors are normalized even multivectors (scalar + bivector).",
        methods: [
          {
            name: "fromBivector",
            signature: "static fromBivector(bivector: WasmMultivector, angle: number): WasmRotor",
            description: "Create a rotor from a bivector (rotation plane) and angle",
            isStatic: true,
            parameters: [
              { name: "bivector", type: "WasmMultivector", description: "Bivector defining rotation plane" },
              { name: "angle", type: "number", description: "Rotation angle in radians" }
            ],
            returns: "WasmRotor",
            example: `// Rotation in the xy-plane
const e12 = WasmMultivector.fromCoefficients(new Float64Array([0,0,0,0,1,0,0,0]));
const rotor = WasmRotor.fromBivector(e12, Math.PI / 2); // 90° rotation`
          },
          {
            name: "apply",
            signature: "apply(mv: WasmMultivector): WasmMultivector",
            description: "Apply rotor to transform a multivector: RMR̃",
            parameters: [{ name: "mv", type: "WasmMultivector", description: "Multivector to transform" }],
            returns: "Transformed WasmMultivector"
          },
          {
            name: "compose",
            signature: "compose(other: WasmRotor): WasmRotor",
            description: "Compose two rotors (multiply)",
            parameters: [{ name: "other", type: "WasmRotor", description: "Second rotor" }],
            returns: "Composed WasmRotor"
          },
          {
            name: "inverse",
            signature: "inverse(): WasmRotor",
            description: "Get the inverse rotor (reverses the rotation)",
            returns: "Inverse WasmRotor"
          }
        ]
      },
      {
        name: "BatchOperations",
        description: "High-performance batch operations for multiple multivectors",
        methods: [
          {
            name: "batchGeometricProduct",
            signature: "static batchGeometricProduct(a_batch: Float64Array, b_batch: Float64Array): Float64Array",
            description: "Compute geometric products for multiple pairs in parallel",
            isStatic: true,
            parameters: [
              { name: "a_batch", type: "Float64Array", description: "First batch (n × 8 values)" },
              { name: "b_batch", type: "Float64Array", description: "Second batch (n × 8 values)" }
            ],
            returns: "Float64Array of results (n × 8 values)"
          },
          {
            name: "batchAdd",
            signature: "static batchAdd(a_batch: Float64Array, b_batch: Float64Array): Float64Array",
            description: "Add multiple multivector pairs",
            isStatic: true
          }
        ]
      },
      {
        name: "PerformanceOperations",
        description: "Optimized vector operations with memory pooling",
        methods: [
          {
            name: "vectorDotProduct",
            signature: "static vectorDotProduct(v1: Float64Array, v2: Float64Array): number",
            description: "Optimized dot product",
            isStatic: true
          },
          {
            name: "vectorCrossProduct",
            signature: "static vectorCrossProduct(v1: Float64Array, v2: Float64Array): Float64Array",
            description: "Optimized cross product for 3D vectors",
            isStatic: true
          },
          {
            name: "batchNormalize",
            signature: "static batchNormalize(vectors: Float64Array, vector_size: number): Float64Array",
            description: "Normalize multiple vectors in batch",
            isStatic: true
          }
        ]
      }
    ]
  },
  {
    id: "tropical",
    title: "Tropical Algebra",
    description: "Max-plus semiring operations for optimization, shortest paths, and neural network efficiency",
    icon: "⊕",
    classes: [
      {
        name: "WasmTropicalNumber",
        description: "Tropical (max-plus) number where ⊕ = max and ⊗ = +. Useful for optimization and path-finding.",
        methods: [
          {
            name: "constructor",
            signature: "new WasmTropicalNumber(value: number)",
            description: "Create a tropical number from a real value",
            parameters: [{ name: "value", type: "number", description: "Real value" }],
            example: `const a = new WasmTropicalNumber(3);
const b = new WasmTropicalNumber(5);
console.log(a.tropicalAdd(b).getValue()); // max(3, 5) = 5
console.log(a.tropicalMul(b).getValue()); // 3 + 5 = 8`
          },
          {
            name: "zero",
            signature: "static zero(): WasmTropicalNumber",
            description: "Create tropical zero (-∞), the additive identity",
            isStatic: true
          },
          {
            name: "one",
            signature: "static one(): WasmTropicalNumber",
            description: "Create tropical one (0), the multiplicative identity",
            isStatic: true
          },
          {
            name: "fromLogProb",
            signature: "static fromLogProb(log_p: number): WasmTropicalNumber",
            description: "Create from log probability (natural for softmax)",
            isStatic: true
          },
          {
            name: "tropicalAdd",
            signature: "tropicalAdd(other: WasmTropicalNumber): WasmTropicalNumber",
            description: "Tropical addition: max(a, b)",
            parameters: [{ name: "other", type: "WasmTropicalNumber", description: "Other operand" }]
          },
          {
            name: "tropicalMul",
            signature: "tropicalMul(other: WasmTropicalNumber): WasmTropicalNumber",
            description: "Tropical multiplication: a + b",
            parameters: [{ name: "other", type: "WasmTropicalNumber", description: "Other operand" }]
          },
          {
            name: "tropicalPow",
            signature: "tropicalPow(n: number): WasmTropicalNumber",
            description: "Tropical power: n × a",
            parameters: [{ name: "n", type: "number", description: "Exponent" }]
          },
          {
            name: "toProb",
            signature: "toProb(): number",
            description: "Convert back to probability via exp()"
          },
          {
            name: "getValue",
            signature: "getValue(): number",
            description: "Get the underlying value"
          },
          {
            name: "isZero",
            signature: "isZero(): boolean",
            description: "Check if tropical zero (-∞)"
          },
          {
            name: "isInfinity",
            signature: "isInfinity(): boolean",
            description: "Check if infinite"
          }
        ]
      },
      {
        name: "WasmTropicalPolynomial",
        description: "Polynomial operations in tropical algebra",
        methods: [
          {
            name: "constructor",
            signature: "new WasmTropicalPolynomial(coefficients: Float64Array)",
            description: "Create from coefficients",
            example: `const poly = new WasmTropicalPolynomial(new Float64Array([1, 2, 3]));`
          },
          {
            name: "evaluate",
            signature: "evaluate(x: WasmTropicalNumber): WasmTropicalNumber",
            description: "Evaluate polynomial at a tropical point"
          },
          {
            name: "tropical_roots",
            signature: "tropical_roots(): Array<any>",
            description: "Find tropical roots (corners of the tropical curve)"
          }
        ]
      },
      {
        name: "WasmTropicalCurve",
        description: "Tropical curves for enumerative geometry",
        methods: [
          {
            name: "constructor",
            signature: "new WasmTropicalCurve(degree: number, genus: number)",
            description: "Create a tropical curve",
            parameters: [
              { name: "degree", type: "number", description: "Curve degree" },
              { name: "genus", type: "number", description: "Curve genus" }
            ]
          },
          {
            name: "expectedVertices",
            signature: "expectedVertices(): number",
            description: "Compute expected vertices using Euler characteristic"
          }
        ]
      },
      {
        name: "WasmTropicalViterbi",
        description: "Viterbi algorithm using tropical algebra for HMM decoding",
        methods: [
          {
            name: "constructor",
            signature: "new WasmTropicalViterbi(transitions: any, emissions: any)",
            description: "Create Viterbi decoder from transition and emission matrices"
          },
          {
            name: "decode",
            signature: "decode(observations: Uint32Array): any",
            description: "Decode most likely state sequence",
            returns: "Object with states and log probability"
          },
          {
            name: "forward_probabilities",
            signature: "forward_probabilities(observations: Uint32Array): Array<any>",
            description: "Compute forward probabilities"
          }
        ]
      },
      {
        name: "TropicalBatch",
        description: "Batch tropical operations for ML workloads, including 0.21.0 semiring folds",
        methods: [
          {
            name: "maxLogProb",
            signature: "static maxLogProb(log_probs: Float64Array): number",
            description: "Find maximum log probability (tropical sum)",
            isStatic: true
          },
          {
            name: "viterbiStep",
            signature: "static viterbiStep(prev_scores: Float64Array, transition_scores: Float64Array, emission_scores: Float64Array, num_states: number): Float64Array",
            description: "Single Viterbi step for batch processing",
            isStatic: true
          },
          {
            name: "batchTropicalAdd",
            signature: "static batchTropicalAdd(values: Float64Array): number",
            description: "Batch tropical addition (max over all values)",
            isStatic: true
          },
          {
            name: "foldOplus",
            signature: "static foldOplus(values: Float64Array): number",
            description: "Fold values with tropical addition / best-choice max-plus aggregation",
            isStatic: true,
            example: `const best = TropicalBatch.foldOplus(new Float64Array([3, 5, 2]));
console.log(best); // 5`
          },
          {
            name: "foldOtimes",
            signature: "static foldOtimes(values: Float64Array): number",
            description: "Fold values with tropical multiplication / max-plus composition",
            isStatic: true,
            example: `const composed = TropicalBatch.foldOtimes(new Float64Array([3, 5, 2]));
console.log(composed); // 10`
          }
        ]
      },
      {
        name: "WasmOrdinalArena",
        description: "0.21.0 arena-backed ordinal handles below ε₀ for ordinal-weighted tropical optimization",
        methods: [
          {
            name: "constructor",
            signature: "new WasmOrdinalArena()",
            description: "Create an ordinal arena that owns all ordinal handles"
          },
          {
            name: "finite",
            signature: "finite(n: number): WasmOrdinal",
            description: "Create a finite ordinal"
          },
          {
            name: "omega",
            signature: "omega(): WasmOrdinal",
            description: "Create ω, the first infinite ordinal below ε₀"
          },
          {
            name: "add",
            signature: "add(left: WasmOrdinal, right: WasmOrdinal): WasmOrdinal",
            description: "Ordinal addition inside the arena"
          },
          {
            name: "formatOrdinal",
            signature: "formatOrdinal(ordinal: WasmOrdinal): string",
            description: "Format an ordinal handle for display"
          },
          {
            name: "weightFromOrdinal",
            signature: "weightFromOrdinal(ordinal: WasmOrdinal): WasmOrdinalWeight",
            description: "Lift an ordinal into an ordinal tropical weight"
          },
          {
            name: "oplusWeight",
            signature: "oplusWeight(left: WasmOrdinalWeight, right: WasmOrdinalWeight): WasmOrdinalWeight",
            description: "Best-choice tropical addition for ordinal weights"
          },
          {
            name: "otimesWeight",
            signature: "otimesWeight(left: WasmOrdinalWeight, right: WasmOrdinalWeight): WasmOrdinalWeight",
            description: "Composition tropical multiplication for ordinal weights"
          },
          {
            name: "bestWeight2",
            signature: "bestWeight2(left: WasmOrdinalWeight, right: WasmOrdinalWeight): WasmOrdinalWeight",
            description: "Small fixed-arity best-weight helper for browser callers"
          },
          {
            name: "composeWeight2",
            signature: "composeWeight2(left: WasmOrdinalWeight, right: WasmOrdinalWeight): WasmOrdinalWeight",
            description: "Small fixed-arity compose-weight helper for browser callers"
          }
        ]
      },
      {
        name: "WasmOrdinalWeight",
        description: "0.21.0 ordinal tropical weight wrapper with zero, one, bottom, and ordinal variants",
        methods: [
          {
            name: "zero",
            signature: "static zero(): WasmOrdinalWeight",
            description: "Create the additive identity",
            isStatic: true
          },
          {
            name: "one",
            signature: "static one(): WasmOrdinalWeight",
            description: "Create the multiplicative identity",
            isStatic: true
          },
          {
            name: "bottom",
            signature: "static bottom(): WasmOrdinalWeight",
            description: "Create the bottom / impossible weight",
            isStatic: true
          },
          {
            name: "ordinalIndex",
            signature: "ordinalIndex(): number | undefined",
            description: "Return the backing ordinal arena index when this is an ordinal weight"
          }
        ]
      },
      {
        name: "TropicalMLOps",
        description: "Tropical algebra for machine learning optimization",
        methods: [
          {
            name: "shortestPaths",
            signature: "static shortestPaths(distance_matrix: any): Array<any>",
            description: "All-pairs shortest paths using Floyd-Warshall",
            isStatic: true
          },
          {
            name: "matrixMultiply",
            signature: "static matrixMultiply(a: any, b: any): Array<any>",
            description: "Tropical matrix multiplication",
            isStatic: true
          }
        ]
      }
    ]
  },
  {
    id: "dual",
    title: "Dual Numbers & Autodiff",
    description: "Automatic differentiation using dual numbers for exact gradients",
    icon: "∂",
    classes: [
      {
        name: "WasmDualNumber",
        description: "Dual number a + bε where ε² = 0. Enables forward-mode automatic differentiation.",
        methods: [
          {
            name: "constructor",
            signature: "new WasmDualNumber(real: number, dual: number)",
            description: "Create a dual number with real and dual parts",
            example: `const x = new WasmDualNumber(3.0, 1.0); // x = 3 + ε (variable)
const c = new WasmDualNumber(2.0, 0.0); // c = 2 (constant)`
          },
          {
            name: "variable",
            signature: "static variable(value: number): WasmDualNumber",
            description: "Create a variable (dual part = 1) for differentiation",
            isStatic: true,
            example: `const x = WasmDualNumber.variable(2.0);
const y = x.mul(x); // y = x²
console.log(y.getReal()); // 4.0 (value)
console.log(y.getDual()); // 4.0 (derivative: 2x at x=2)`
          },
          {
            name: "constant",
            signature: "static constant(value: number): WasmDualNumber",
            description: "Create a constant (dual part = 0)",
            isStatic: true
          },
          {
            name: "add",
            signature: "add(other: WasmDualNumber): WasmDualNumber",
            description: "Addition with derivative tracking"
          },
          {
            name: "sub",
            signature: "sub(other: WasmDualNumber): WasmDualNumber",
            description: "Subtraction with derivative tracking"
          },
          {
            name: "mul",
            signature: "mul(other: WasmDualNumber): WasmDualNumber",
            description: "Multiplication: d(uv) = u'v + uv'"
          },
          {
            name: "div",
            signature: "div(other: WasmDualNumber): WasmDualNumber",
            description: "Division: d(u/v) = (u'v - uv')/v²"
          },
          {
            name: "sin",
            signature: "sin(): WasmDualNumber",
            description: "Sine with derivative (cos)"
          },
          {
            name: "cos",
            signature: "cos(): WasmDualNumber",
            description: "Cosine with derivative (-sin)"
          },
          {
            name: "tan",
            signature: "tan(): WasmDualNumber",
            description: "Tangent with derivative (sec²)"
          },
          {
            name: "exp",
            signature: "exp(): WasmDualNumber",
            description: "Exponential with derivative (exp)"
          },
          {
            name: "ln",
            signature: "ln(): WasmDualNumber",
            description: "Natural log with derivative (1/x)"
          },
          {
            name: "sqrt",
            signature: "sqrt(): WasmDualNumber",
            description: "Square root with derivative (1/(2√x))"
          },
          {
            name: "pow",
            signature: "pow(exponent: number): WasmDualNumber",
            description: "Power with derivative (n·x^(n-1))"
          },
          {
            name: "sigmoid",
            signature: "sigmoid(): WasmDualNumber",
            description: "Sigmoid activation: σ(x) = 1/(1+e^(-x))"
          },
          {
            name: "tanh",
            signature: "tanh(): WasmDualNumber",
            description: "Hyperbolic tangent activation"
          },
          {
            name: "relu",
            signature: "relu(): WasmDualNumber",
            description: "ReLU activation: max(0, x)"
          },
          {
            name: "softplus",
            signature: "softplus(): WasmDualNumber",
            description: "Softplus activation: ln(1 + e^x)"
          },
          {
            name: "maxByPolicy",
            signature: "maxByPolicy(other: WasmDualNumber, policy: WasmBranchPolicy): WasmDualNumber",
            description: "0.21.0 branch-sensitive maximum with explicit derivative tie handling"
          },
          {
            name: "minByPolicy",
            signature: "minByPolicy(other: WasmDualNumber, policy: WasmBranchPolicy): WasmDualNumber",
            description: "0.21.0 branch-sensitive minimum with explicit derivative tie handling"
          },
          {
            name: "getReal",
            signature: "getReal(): number",
            description: "Get the function value"
          },
          {
            name: "getDual",
            signature: "getDual(): number",
            description: "Get the derivative"
          }
        ]
      },
      {
        name: "WasmBranchPolicy",
        description: "0.21.0 enum controlling derivative tie behavior at non-smooth min/max branch points",
        methods: [
          {
            name: "Left",
            signature: "WasmBranchPolicy.Left",
            description: "Preserve the left operand derivative on ties"
          },
          {
            name: "Right",
            signature: "WasmBranchPolicy.Right",
            description: "Preserve the right operand derivative on ties"
          },
          {
            name: "Average",
            signature: "WasmBranchPolicy.Average",
            description: "Average operand derivatives on ties"
          }
        ]
      },
      {
        name: "WasmMultiDualNumber",
        description: "Multi-variable dual numbers for computing gradients of functions with multiple inputs",
        methods: [
          {
            name: "constructor",
            signature: "new WasmMultiDualNumber(real: number, duals: Float64Array)",
            description: "Create with value and partial derivatives"
          },
          {
            name: "variable",
            signature: "static variable(value: number, num_vars: number, var_index: number): WasmMultiDualNumber",
            description: "Create a variable for the i-th input",
            isStatic: true,
            example: `// Compute gradient of f(x,y) = x² + xy at (3, 4)
const x = WasmMultiDualNumber.variable(3, 2, 0);
const y = WasmMultiDualNumber.variable(4, 2, 1);
const f = x.mul(x).add(x.mul(y));
console.log(f.getGradient()); // [2x+y, x] = [10, 3]`
          },
          {
            name: "constant",
            signature: "static constant(value: number, num_vars: number): WasmMultiDualNumber",
            description: "Create a constant",
            isStatic: true
          },
          {
            name: "variables",
            signature: "static variables(values: Float64Array): Array<WasmMultiDualNumber>",
            description: "0.21.0 helper that creates one basis-seeded variable per input coordinate",
            isStatic: true,
            example: `const [x, y] = WasmMultiDualNumber.variables(new Float64Array([2, 3]));
const f = x.mul(x).add(x.mul(y));
console.log(f.getGradient()); // [7, 2]`
          },
          {
            name: "maxByPolicy",
            signature: "maxByPolicy(other: WasmMultiDualNumber, policy: WasmBranchPolicy): WasmMultiDualNumber",
            description: "0.21.0 branch-sensitive maximum with explicit gradient tie handling"
          },
          {
            name: "minByPolicy",
            signature: "minByPolicy(other: WasmMultiDualNumber, policy: WasmBranchPolicy): WasmMultiDualNumber",
            description: "0.21.0 branch-sensitive minimum with explicit gradient tie handling"
          },
          {
            name: "getGradient",
            signature: "getGradient(): Float64Array",
            description: "Get all partial derivatives"
          },
          {
            name: "getPartial",
            signature: "getPartial(index: number): number",
            description: "Get specific partial derivative"
          }
        ]
      },
      {
        name: "WasmStaticMultiDual2",
        description: "0.21.0 fixed-size two-variable forward-mode AD wrapper for small hot loops",
        methods: [
          {
            name: "constructor",
            signature: "new WasmStaticMultiDual2(value: number, gradient: Float64Array)",
            description: "Create a fixed-size multi-dual number from a value and two partial derivatives"
          },
          {
            name: "variable",
            signature: "static variable(value: number, var_index: number): WasmStaticMultiDual2",
            description: "Create a basis-seeded fixed-size variable",
            isStatic: true
          },
          {
            name: "variables",
            signature: "static variables(values: Float64Array): Array<WasmStaticMultiDual2>",
            description: "Create one fixed-size variable per coordinate",
            isStatic: true
          },
          {
            name: "add",
            signature: "add(other: WasmStaticMultiDual2): WasmStaticMultiDual2",
            description: "Fixed-size addition"
          },
          {
            name: "mul",
            signature: "mul(other: WasmStaticMultiDual2): WasmStaticMultiDual2",
            description: "Fixed-size multiplication with gradient propagation"
          },
          {
            name: "maxByPolicy",
            signature: "maxByPolicy(other: WasmStaticMultiDual2, policy: WasmBranchPolicy): WasmStaticMultiDual2",
            description: "Branch-sensitive maximum with explicit gradient tie handling"
          },
          {
            name: "toMultiDual",
            signature: "toMultiDual(): WasmMultiDualNumber",
            description: "Convert to the heap-backed multi-dual wrapper"
          }
        ]
      },
      {
        name: "WasmStaticMultiDual3",
        description: "0.21.0 fixed-size three-variable forward-mode AD wrapper",
        methods: [
          {
            name: "variable",
            signature: "static variable(value: number, var_index: number): WasmStaticMultiDual3",
            description: "Create a basis-seeded fixed-size variable",
            isStatic: true
          },
          {
            name: "variables",
            signature: "static variables(values: Float64Array): Array<WasmStaticMultiDual3>",
            description: "Create one fixed-size variable per coordinate",
            isStatic: true
          },
          {
            name: "toMultiDual",
            signature: "toMultiDual(): WasmMultiDualNumber",
            description: "Convert to the heap-backed multi-dual wrapper"
          }
        ]
      },
      {
        name: "WasmStaticMultiDual4",
        description: "0.21.0 fixed-size four-variable forward-mode AD wrapper",
        methods: [
          {
            name: "variable",
            signature: "static variable(value: number, var_index: number): WasmStaticMultiDual4",
            description: "Create a basis-seeded fixed-size variable",
            isStatic: true
          },
          {
            name: "variables",
            signature: "static variables(values: Float64Array): Array<WasmStaticMultiDual4>",
            description: "Create one fixed-size variable per coordinate",
            isStatic: true
          },
          {
            name: "toMultiDual",
            signature: "toMultiDual(): WasmMultiDualNumber",
            description: "Convert to the heap-backed multi-dual wrapper"
          }
        ]
      },
      {
        name: "AutoDiff",
        description: "Automatic differentiation utilities for neural networks",
        methods: [
          {
            name: "linearLayer",
            signature: "static linearLayer(inputs: Float64Array, weights: Float64Array, bias: Float64Array, input_size: number, output_size: number): Float64Array",
            description: "Forward pass of linear layer with gradients",
            isStatic: true
          },
          {
            name: "meanSquaredError",
            signature: "static meanSquaredError(predictions: Float64Array, targets: Float64Array): WasmDualNumber",
            description: "MSE loss with gradient",
            isStatic: true
          },
          {
            name: "evaluatePolynomial",
            signature: "static evaluatePolynomial(x: number, coefficients: Float64Array): WasmDualNumber",
            description: "Evaluate polynomial with derivative",
            isStatic: true
          }
        ]
      },
      {
        name: "MLOps",
        description: "Machine learning operations with automatic differentiation",
        methods: [
          {
            name: "batchActivation",
            signature: "static batchActivation(inputs: Float64Array, activation: string): Float64Array",
            description: "Batch activation function ('relu', 'sigmoid', 'tanh')",
            isStatic: true
          },
          {
            name: "crossEntropyLoss",
            signature: "static crossEntropyLoss(predictions: Float64Array, targets: Float64Array): WasmDualNumber",
            description: "Cross-entropy loss with gradient",
            isStatic: true
          },
          {
            name: "gradientDescentStep",
            signature: "static gradientDescentStep(parameters: Float64Array, gradients: Float64Array, learning_rate: number): Float64Array",
            description: "Single gradient descent update",
            isStatic: true
          },
          {
            name: "softmax",
            signature: "static softmax(inputs: Float64Array): Float64Array",
            description: "Softmax normalization",
            isStatic: true
          }
        ]
      }
    ]
  },
  {
    id: "info-geom",
    title: "Information Geometry",
    description: "Statistical manifolds, Fisher metrics, and divergences",
    icon: "ℐ",
    classes: [
      {
        name: "WasmFisherInformationMatrix",
        description: "Fisher information matrix for statistical inference",
        methods: [
          {
            name: "getEigenvalues",
            signature: "getEigenvalues(): Float64Array",
            description: "Get eigenvalues of the Fisher matrix"
          },
          {
            name: "conditionNumber",
            signature: "conditionNumber(): number",
            description: "Ratio of largest to smallest eigenvalue"
          },
          {
            name: "isPositiveDefinite",
            signature: "isPositiveDefinite(): boolean",
            description: "Check if the matrix is positive definite"
          }
        ]
      },
      {
        name: "WasmDuallyFlatManifold",
        description: "Dually flat manifold for exponential families",
        methods: [
          {
            name: "constructor",
            signature: "new WasmDuallyFlatManifold(dimension: number, alpha: number)",
            description: "Create with dimension and α-connection parameter"
          },
          {
            name: "klDivergence",
            signature: "klDivergence(p: Float64Array, q: Float64Array): number",
            description: "KL divergence D_KL(P||Q)",
            example: `const manifold = new WasmDuallyFlatManifold(3, 0);
const kl = manifold.klDivergence(
  new Float64Array([0.5, 0.3, 0.2]),
  new Float64Array([0.33, 0.33, 0.34])
);`
          },
          {
            name: "jsDivergence",
            signature: "jsDivergence(p: Float64Array, q: Float64Array): number",
            description: "Jensen-Shannon divergence (symmetric)"
          },
          {
            name: "bregmanDivergence",
            signature: "bregmanDivergence(p: Float64Array, q: Float64Array): number",
            description: "Bregman divergence"
          },
          {
            name: "wassersteinDistance",
            signature: "wassersteinDistance(p: Float64Array, q: Float64Array): number",
            description: "Wasserstein-1 distance (Earth Mover's Distance)"
          },
          {
            name: "fisherMetricAt",
            signature: "fisherMetricAt(point: Float64Array): WasmFisherInformationMatrix",
            description: "Compute Fisher metric at a point"
          }
        ]
      },
      {
        name: "WasmAlphaConnection",
        description: "α-connection on statistical manifolds",
        methods: [
          {
            name: "constructor",
            signature: "new WasmAlphaConnection(alpha: number)",
            description: "Create α-connection",
            parameters: [{ name: "alpha", type: "number", description: "α parameter (-1=mixture, 0=Levi-Civita, 1=exponential)" }]
          },
          {
            name: "isExponential",
            signature: "isExponential(): boolean",
            description: "Check if α = 1"
          },
          {
            name: "isMixture",
            signature: "isMixture(): boolean",
            description: "Check if α = -1"
          },
          {
            name: "isLeviCivita",
            signature: "isLeviCivita(): boolean",
            description: "Check if α = 0"
          }
        ]
      },
      {
        name: "InfoGeomUtils",
        description: "Information geometry utilities",
        methods: [
          {
            name: "entropy",
            signature: "static entropy(p: Float64Array): number",
            description: "Shannon entropy H(p) = -Σp log p",
            isStatic: true
          },
          {
            name: "crossEntropy",
            signature: "static crossEntropy(p: Float64Array, q: Float64Array): number",
            description: "Cross-entropy H(p,q) = -Σp log q",
            isStatic: true
          },
          {
            name: "mutualInformation",
            signature: "static mutualInformation(joint: Float64Array, marginal_x: Float64Array, marginal_y: Float64Array, dim_x: number): number",
            description: "Mutual information I(X;Y)",
            isStatic: true
          },
          {
            name: "softmax",
            signature: "static softmax(logits: Float64Array): Float64Array",
            description: "Softmax normalization",
            isStatic: true
          },
          {
            name: "randomSimplex",
            signature: "static randomSimplex(dimension: number): Float64Array",
            description: "Generate random point on probability simplex",
            isStatic: true
          }
        ]
      }
    ]
  },
  {
    id: "calculus",
    title: "Calculus & Integration",
    description: "Differential operators, numerical integration, and field operations",
    icon: "∫",
    classes: [
      {
        name: "ScalarField",
        description: "Scalar field f: ℝⁿ → ℝ for calculus operations",
        methods: [
          {
            name: "fromFunction2D",
            signature: "static fromFunction2D(func: Function): ScalarField",
            description: "Create 2D scalar field from JavaScript function",
            isStatic: true,
            example: `const field = ScalarField.fromFunction2D((x, y) => x*x + y*y);
const value = field.evaluate(new Float64Array([1.0, 2.0])); // 5.0`
          },
          {
            name: "fromFunction3D",
            signature: "static fromFunction3D(func: Function): ScalarField",
            description: "Create 3D scalar field",
            isStatic: true
          },
          {
            name: "evaluate",
            signature: "evaluate(point: Float64Array): number",
            description: "Evaluate field at a point"
          },
          {
            name: "batchEvaluate",
            signature: "batchEvaluate(points: Float64Array): Float64Array",
            description: "Evaluate at multiple points"
          }
        ]
      },
      {
        name: "VectorField",
        description: "Vector field F: ℝⁿ → ℝⁿ for differential operations",
        methods: [
          {
            name: "fromFunction2D",
            signature: "static fromFunction2D(func: Function): VectorField",
            description: "Create 2D vector field",
            isStatic: true,
            example: `// Rotation field
const field = VectorField.fromFunction2D((x, y) => [y, -x]);
const vec = field.evaluate(new Float64Array([1.0, 2.0])); // [2.0, -1.0]`
          },
          {
            name: "fromFunction3D",
            signature: "static fromFunction3D(func: Function): VectorField",
            description: "Create 3D vector field",
            isStatic: true
          },
          {
            name: "evaluate",
            signature: "evaluate(point: Float64Array): Float64Array",
            description: "Evaluate field at a point"
          }
        ]
      },
      {
        name: "NumericalDerivative",
        description: "Numerical differentiation operations using finite differences",
        methods: [
          {
            name: "constructor",
            signature: "new NumericalDerivative(step_size?: number)",
            description: "Create with optional step size (default: 1e-5)"
          },
          {
            name: "gradient",
            signature: "gradient(field: ScalarField, point: Float64Array): Float64Array",
            description: "Compute gradient ∇f = [∂f/∂x, ∂f/∂y, ...]",
            example: `const field = ScalarField.fromFunction2D((x, y) => x*x + y*y);
const derivative = new NumericalDerivative();
const grad = derivative.gradient(field, new Float64Array([1.0, 2.0]));
// Returns [2.0, 4.0] (gradient at (1,2))`
          },
          {
            name: "divergence",
            signature: "divergence(field: VectorField, point: Float64Array): number",
            description: "Compute divergence ∇·F = ∂Fx/∂x + ∂Fy/∂y + ..."
          },
          {
            name: "curl",
            signature: "curl(field: VectorField, point: Float64Array): Float64Array",
            description: "Compute curl ∇×F (3D only)"
          },
          {
            name: "laplacian",
            signature: "laplacian(field: ScalarField, point: Float64Array): number",
            description: "Compute Laplacian ∇²f = ∂²f/∂x² + ∂²f/∂y² + ..."
          }
        ]
      },
      {
        name: "Integration",
        description: "Numerical integration using Simpson's rule",
        methods: [
          {
            name: "integrate1D",
            signature: "static integrate1D(func: Function, a: number, b: number, n: number): number",
            description: "1D integral ∫[a,b] f(x) dx",
            isStatic: true,
            example: `// Integrate x² from 0 to 1
const result = Integration.integrate1D(x => x*x, 0, 1, 100);
// Returns ≈ 0.333...`
          },
          {
            name: "integrate2D",
            signature: "static integrate2D(func: Function, ax: number, bx: number, ay: number, by: number, nx: number, ny: number): number",
            description: "2D integral ∫∫ f(x,y) dx dy",
            isStatic: true
          }
        ]
      },
      {
        name: "RiemannianManifold",
        description: "Curved space with metric tensor for geodesics and curvature",
        methods: [
          {
            name: "sphere",
            signature: "static sphere(radius: number): RiemannianManifold",
            description: "Create a 2D sphere with given radius",
            isStatic: true
          },
          {
            name: "hyperbolic",
            signature: "static hyperbolic(): RiemannianManifold",
            description: "Create 2D hyperbolic plane (Poincaré half-plane)",
            isStatic: true
          },
          {
            name: "euclidean",
            signature: "static euclidean(dimension: number): RiemannianManifold",
            description: "Create flat Euclidean space",
            isStatic: true
          },
          {
            name: "scalarCurvature",
            signature: "scalarCurvature(coords: Float64Array): number",
            description: "Compute scalar curvature R"
          },
          {
            name: "christoffel",
            signature: "christoffel(k: number, i: number, j: number, coords: Float64Array): number",
            description: "Compute Christoffel symbol Γᵏᵢⱼ"
          },
          {
            name: "geodesic",
            signature: "geodesic(initial_pos: Float64Array, initial_vel: Float64Array, t_max: number, dt: number): Float64Array",
            description: "Solve geodesic equations using RK4"
          }
        ]
      }
    ]
  },
  {
    id: "measure",
    title: "Measure Theory",
    description: "Measures, probability spaces, and parametric densities",
    icon: "μ",
    classes: [
      {
        name: "WasmLebesgueMeasure",
        description: "Lebesgue measure generalizing length, area, and volume",
        methods: [
          {
            name: "constructor",
            signature: "new WasmLebesgueMeasure(dimension: number)",
            description: "Create for given dimension (1=length, 2=area, 3=volume)"
          },
          {
            name: "measureInterval",
            signature: "measureInterval(lower: Float64Array, upper: Float64Array): number",
            description: "Measure of a box [a,b] × [c,d] × ..."
          },
          {
            name: "measureBox",
            signature: "measureBox(sides: Float64Array): number",
            description: "Measure of a box with given side lengths"
          }
        ]
      },
      {
        name: "WasmCountingMeasure",
        description: "Counting measure - assigns cardinality to sets",
        methods: [
          {
            name: "constructor",
            signature: "new WasmCountingMeasure()",
            description: "Create counting measure"
          },
          {
            name: "measureFiniteSet",
            signature: "measureFiniteSet(set_size: number): number",
            description: "Returns set cardinality"
          }
        ]
      },
      {
        name: "WasmProbabilityMeasure",
        description: "Probability measure with total mass 1",
        methods: [
          {
            name: "constructor",
            signature: "new WasmProbabilityMeasure()",
            description: "Create uniform probability measure on [0,1]"
          },
          {
            name: "uniform",
            signature: "static uniform(a: number, b: number): WasmProbabilityMeasure",
            description: "Create uniform measure on [a,b]",
            isStatic: true
          },
          {
            name: "probabilityInterval",
            signature: "probabilityInterval(a: number, b: number, lower: number, upper: number): number",
            description: "Compute P(X ∈ [a,b])"
          }
        ]
      },
      {
        name: "WasmParametricDensity",
        description: "Parametric probability density families with Fisher information",
        methods: [
          {
            name: "gaussian",
            signature: "static gaussian(): WasmParametricDensity",
            description: "Gaussian density N(μ, σ²)",
            isStatic: true,
            example: `const gaussian = WasmParametricDensity.gaussian();
const params = new Float64Array([0, 1]); // μ=0, σ=1
const pdf = gaussian.evaluate(0.5, params);`
          },
          {
            name: "exponential",
            signature: "static exponential(): WasmParametricDensity",
            description: "Exponential density Exp(λ)",
            isStatic: true
          },
          {
            name: "laplace",
            signature: "static laplace(): WasmParametricDensity",
            description: "Laplace density Laplace(μ, b)",
            isStatic: true
          },
          {
            name: "cauchy",
            signature: "static cauchy(): WasmParametricDensity",
            description: "Cauchy density Cauchy(x₀, γ)",
            isStatic: true
          },
          {
            name: "evaluate",
            signature: "evaluate(x: number, params: Float64Array): number",
            description: "Evaluate density at x with parameters"
          },
          {
            name: "logDensity",
            signature: "logDensity(x: number, params: Float64Array): number",
            description: "Log-density log p(x|θ)"
          },
          {
            name: "fisherInformation",
            signature: "fisherInformation(data: Float64Array, params: Float64Array): Float64Array",
            description: "Compute Fisher information matrix"
          }
        ]
      },
      {
        name: "WasmFisherMeasure",
        description: "Fisher-Riemannian geometry on statistical manifolds",
        methods: [
          {
            name: "fromDensity",
            signature: "static fromDensity(density: WasmParametricDensity): WasmFisherMeasure",
            description: "Create from parametric density",
            isStatic: true
          },
          {
            name: "fisherMetric",
            signature: "fisherMetric(data: Float64Array, params: Float64Array): Float64Array",
            description: "Compute Fisher metric at parameter θ"
          },
          {
            name: "volumeElement",
            signature: "volumeElement(data: Float64Array, params: Float64Array): number",
            description: "Riemannian volume √det(g(θ))"
          }
        ]
      },
      {
        name: "WasmTropicalMeasure",
        description: "Tropical (max-plus) measure for optimization",
        methods: [
          {
            name: "supremum",
            signature: "supremum(f: Function, points: Float64Array): Float64Array",
            description: "Tropical supremum (max) over sample points"
          },
          {
            name: "infimum",
            signature: "infimum(f: Function, points: Float64Array): Float64Array",
            description: "Tropical infimum (min) over sample points"
          },
          {
            name: "tropicalIntegrate",
            signature: "tropicalIntegrate(f: Function, a: number, b: number, samples: number): number",
            description: "Tropical integration (supremum over region)"
          }
        ]
      }
    ]
  },
  {
    id: "probabilistic",
    title: "Probabilistic Computing",
    description: "Distributions, MCMC sampling, and stochastic processes on multivector spaces",
    icon: "𝒫",
    classes: [
      {
        name: "WasmGaussianMultivector",
        description: "Gaussian distribution over 8-dimensional multivector space",
        methods: [
          {
            name: "constructor",
            signature: "new WasmGaussianMultivector()",
            description: "Create standard Gaussian (zero mean, unit variance)"
          },
          {
            name: "withParameters",
            signature: "static withParameters(mean: Float64Array, std_dev: Float64Array): WasmGaussianMultivector",
            description: "Create with specified mean and standard deviation (8 values each)",
            isStatic: true
          },
          {
            name: "isotropic",
            signature: "static isotropic(mean: Float64Array, variance: number): WasmGaussianMultivector",
            description: "Create isotropic Gaussian (same variance for all components)",
            isStatic: true
          },
          {
            name: "gradeConcentrated",
            signature: "static gradeConcentrated(grade: number, variance: number): WasmGaussianMultivector",
            description: "Gaussian concentrated on specific grade (0-3)",
            isStatic: true
          },
          {
            name: "sample",
            signature: "sample(): Float64Array",
            description: "Draw a sample (8 coefficients)"
          },
          {
            name: "sampleBatch",
            signature: "sampleBatch(num_samples: number): Float64Array",
            description: "Draw multiple samples (n × 8 values)"
          },
          {
            name: "logProb",
            signature: "logProb(coefficients: Float64Array): number",
            description: "Log probability of a multivector"
          },
          {
            name: "getMean",
            signature: "getMean(): Float64Array",
            description: "Get mean (8 values)"
          },
          {
            name: "getCovariance",
            signature: "getCovariance(): Float64Array",
            description: "Get covariance matrix (64 values, row-major)"
          }
        ]
      },
      {
        name: "WasmMetropolisHastings",
        description: "Metropolis-Hastings MCMC sampler for multivector distributions",
        methods: [
          {
            name: "constructor",
            signature: "new WasmMetropolisHastings(target: WasmGaussianMultivector, proposal_std: number)",
            description: "Create sampler for target distribution",
            example: `const target = WasmGaussianMultivector.withParameters(
  new Float64Array([1, 0, 0, 0, 0, 0, 0, 0]),
  new Float64Array([0.1, 0.1, 0.1, 0.1, 0.1, 0.1, 0.1, 0.1])
);
const sampler = new WasmMetropolisHastings(target, 0.5);
const samples = sampler.run(1000, 100); // 1000 samples, 100 burn-in`
          },
          {
            name: "run",
            signature: "run(num_samples: number, burnin: number): Float64Array",
            description: "Run sampler and collect samples"
          },
          {
            name: "step",
            signature: "step(): Float64Array",
            description: "Take single MCMC step"
          },
          {
            name: "diagnostics",
            signature: "diagnostics(): WasmMCMCDiagnostics",
            description: "Get convergence diagnostics"
          },
          {
            name: "getAcceptanceRate",
            signature: "getAcceptanceRate(): number",
            description: "Current acceptance rate"
          }
        ]
      },
      {
        name: "WasmMCMCDiagnostics",
        description: "MCMC convergence diagnostics",
        methods: [
          {
            name: "isConverged",
            signature: "isConverged(): boolean",
            description: "Check if R-hat < 1.1"
          },
          {
            name: "getRHat",
            signature: "getRHat(): number",
            description: "Gelman-Rubin R-hat statistic"
          },
          {
            name: "getEffectiveSampleSize",
            signature: "getEffectiveSampleSize(): number",
            description: "Effective sample size"
          }
        ]
      },
      {
        name: "WasmGeometricBrownianMotion",
        description: "Geometric Brownian motion: dX = μX dt + σX dW",
        methods: [
          {
            name: "constructor",
            signature: "new WasmGeometricBrownianMotion(mu: number, sigma: number)",
            description: "Create GBM with drift μ and volatility σ"
          },
          {
            name: "samplePath",
            signature: "samplePath(initial: Float64Array, t_end: number, num_steps: number): Float64Array",
            description: "Sample a path: [(t, coeffs), ...]"
          },
          {
            name: "expectedValue",
            signature: "expectedValue(initial: Float64Array, t: number): Float64Array",
            description: "E[X(t)] = X(0)·exp(μt)"
          },
          {
            name: "variance",
            signature: "variance(initial: Float64Array, t: number): Float64Array",
            description: "Var(X(t))"
          }
        ]
      },
      {
        name: "WasmWienerProcess",
        description: "Standard Brownian motion W(t) with W(0) = 0",
        methods: [
          {
            name: "constructor",
            signature: "new WasmWienerProcess(dim?: number)",
            description: "Create Wiener process (default dim=8 for multivectors)"
          },
          {
            name: "samplePath",
            signature: "samplePath(t_end: number, num_steps: number): Float64Array",
            description: "Sample a path: [(t, w), ...]"
          }
        ]
      },
      {
        name: "WasmUncertainMultivector",
        description: "Multivector with uncertainty (mean + covariance) for error propagation",
        methods: [
          {
            name: "constructor",
            signature: "new WasmUncertainMultivector(mean: Float64Array, variances: Float64Array)",
            description: "Create with diagonal covariance"
          },
          {
            name: "withCovariance",
            signature: "static withCovariance(mean: Float64Array, covariance: Float64Array): WasmUncertainMultivector",
            description: "Create with full 8×8 covariance matrix",
            isStatic: true
          },
          {
            name: "deterministic",
            signature: "static deterministic(value: Float64Array): WasmUncertainMultivector",
            description: "Create with zero uncertainty",
            isStatic: true
          },
          {
            name: "add",
            signature: "add(other: WasmUncertainMultivector): WasmUncertainMultivector",
            description: "Add assuming independence"
          },
          {
            name: "scale",
            signature: "scale(scalar: number): WasmUncertainMultivector",
            description: "Linear propagation: Var(aX) = a²Var(X)"
          },
          {
            name: "getMean",
            signature: "getMean(): Float64Array",
            description: "Get mean"
          },
          {
            name: "getStdDevs",
            signature: "getStdDevs(): Float64Array",
            description: "Get standard deviations"
          }
        ]
      },
      {
        name: "WasmMonteCarloEstimator",
        description: "Monte Carlo estimation utilities",
        methods: [
          {
            name: "sampleMean",
            signature: "static sampleMean(samples: Float64Array): Float64Array",
            description: "Compute sample mean from batch",
            isStatic: true
          },
          {
            name: "sampleVariance",
            signature: "static sampleVariance(samples: Float64Array): Float64Array",
            description: "Compute sample variance",
            isStatic: true
          },
          {
            name: "sampleCovariance",
            signature: "static sampleCovariance(samples: Float64Array): Float64Array",
            description: "Compute sample covariance matrix",
            isStatic: true
          }
        ]
      }
    ]
  },
  {
    id: "fusion",
    title: "TropicalDualClifford Fusion",
    description: "Unified algebraic system combining tropical, dual, and Clifford algebras for LLM applications",
    icon: "⊛",
    classes: [
      {
        name: "WasmTropicalDualClifford",
        description: "Unified representation combining tropical (optimization), dual (gradients), and Clifford (geometry) algebras. Designed for LLM embedding manipulation.",
        methods: [
          {
            name: "constructor",
            signature: "new WasmTropicalDualClifford()",
            description: "Create zero TDC object"
          },
          {
            name: "fromLogits",
            signature: "static fromLogits(logits: Float64Array): WasmTropicalDualClifford",
            description: "Create from neural network logits",
            isStatic: true,
            example: `const logits = new Float64Array([1.2, 0.8, -0.5, 0.3, 1.0, 0.2, -0.1, 0.5]);
const tdc = WasmTropicalDualClifford.fromLogits(logits);`
          },
          {
            name: "fromProbabilities",
            signature: "static fromProbabilities(probs: Float64Array): WasmTropicalDualClifford",
            description: "Create from probability distribution",
            isStatic: true
          },
          {
            name: "random",
            signature: "static random(): WasmTropicalDualClifford",
            description: "Create random TDC for testing",
            isStatic: true
          },
          {
            name: "randomVector",
            signature: "static randomVector(): WasmTropicalDualClifford",
            description: "Create random unit vector (grade 1)",
            isStatic: true
          },
          {
            name: "bindingIdentity",
            signature: "static bindingIdentity(): WasmTropicalDualClifford",
            description: "Get binding identity element",
            isStatic: true
          },
          {
            name: "bind",
            signature: "bind(other: WasmTropicalDualClifford): WasmTropicalDualClifford",
            description: "Bind two TDC objects (geometric product) - creates associations",
            example: `// Create key-value association
const key = WasmTropicalDualClifford.randomVector();
const value = WasmTropicalDualClifford.randomVector();
const bound = key.bind(value);`
          },
          {
            name: "unbind",
            signature: "unbind(other: WasmTropicalDualClifford): WasmTropicalDualClifford",
            description: "Unbind: retrieve associated value"
          },
          {
            name: "bundle",
            signature: "bundle(other: WasmTropicalDualClifford, beta: number): WasmTropicalDualClifford",
            description: "Bundle (superposition) - similar to both inputs"
          },
          {
            name: "similarity",
            signature: "similarity(other: WasmTropicalDualClifford): number",
            description: "Compute similarity using Clifford inner product [-1, 1]"
          },
          {
            name: "distance",
            signature: "distance(other: WasmTropicalDualClifford): number",
            description: "Compute distance between TDC systems"
          },
          {
            name: "evaluate",
            signature: "evaluate(other: WasmTropicalDualClifford): WasmEvaluationResult",
            description: "Full evaluation using all three algebras"
          },
          {
            name: "tropicalAttention",
            signature: "tropicalAttention(keys: Float64Array, values: Float64Array): Float64Array",
            description: "Tropical attention operation"
          },
          {
            name: "fusionNorm",
            signature: "fusionNorm(): number",
            description: "Combined norm across all systems"
          },
          {
            name: "interpolate",
            signature: "interpolate(other: WasmTropicalDualClifford, t: number): WasmTropicalDualClifford",
            description: "Interpolate between two TDC objects"
          },
          {
            name: "getTropicalFeatures",
            signature: "getTropicalFeatures(): Float64Array",
            description: "Extract tropical component"
          },
          {
            name: "getDualReals",
            signature: "getDualReals(): Float64Array",
            description: "Extract dual number real parts"
          },
          {
            name: "getCliffordCoefficients",
            signature: "getCliffordCoefficients(): Float64Array",
            description: "Extract Clifford coefficients"
          }
        ]
      },
      {
        name: "FusionBatchOperations",
        description: "Batch operations for LLM workloads",
        methods: [
          {
            name: "batchDistance",
            signature: "static batchDistance(query_logits: Float64Array, corpus_batch: Float64Array, corpus_size: number): Float64Array",
            description: "Batch distance computation for similarity search",
            isStatic: true
          },
          {
            name: "batchTropicalAttention",
            signature: "static batchTropicalAttention(queries: Float64Array, keys: Float64Array, values: Float64Array, query_dim: number): Float64Array",
            description: "Batch tropical attention",
            isStatic: true
          },
          {
            name: "fusionSimilarity",
            signature: "static fusionSimilarity(tdc1: WasmTropicalDualClifford, tdc2: WasmTropicalDualClifford): number",
            description: "Compute fusion similarity",
            isStatic: true
          },
          {
            name: "gradientStep",
            signature: "static gradientStep(tdc: WasmTropicalDualClifford, learning_rate: number): WasmTropicalDualClifford",
            description: "Gradient-based optimization step",
            isStatic: true
          }
        ]
      },
      {
        name: "FusionUtils",
        description: "Conversion utilities",
        methods: [
          {
            name: "validateLogits",
            signature: "static validateLogits(logits: Float64Array): boolean",
            description: "Check logits for numerical stability",
            isStatic: true
          },
          {
            name: "normalizeLogits",
            signature: "static normalizeLogits(logits: Float64Array): Float64Array",
            description: "Normalize to prevent overflow",
            isStatic: true
          },
          {
            name: "softmaxToTropical",
            signature: "static softmaxToTropical(probs: Float64Array): Float64Array",
            description: "Convert softmax to tropical",
            isStatic: true
          },
          {
            name: "tropicalToSoftmax",
            signature: "static tropicalToSoftmax(tropical_values: Float64Array): Float64Array",
            description: "Convert tropical to softmax",
            isStatic: true
          }
        ]
      }
    ]
  },
  {
    id: "network",
    title: "Geometric Networks",
    description: "Network analysis with geometric algebra node representations",
    icon: "⊡",
    classes: [
      {
        name: "WasmGeometricNetwork",
        description: "Network with multivector node positions for geometric analysis",
        methods: [
          {
            name: "constructor",
            signature: "new WasmGeometricNetwork()",
            description: "Create empty network"
          },
          {
            name: "withCapacity",
            signature: "static withCapacity(num_nodes: number, num_edges: number): WasmGeometricNetwork",
            description: "Create with pre-allocated capacity",
            isStatic: true
          },
          {
            name: "addNode",
            signature: "addNode(coefficients: Float64Array): number",
            description: "Add node with multivector position"
          },
          {
            name: "addEdge",
            signature: "addEdge(source: number, target: number, weight: number): void",
            description: "Add directed edge"
          },
          {
            name: "addUndirectedEdge",
            signature: "addUndirectedEdge(a: number, b: number, weight: number): void",
            description: "Add undirected edge (two directed edges)"
          },
          {
            name: "geometricDistance",
            signature: "geometricDistance(node1: number, node2: number): number",
            description: "Compute geometric distance between nodes"
          },
          {
            name: "shortestPath",
            signature: "shortestPath(source: number, target: number): any",
            description: "Find shortest path using edge weights"
          },
          {
            name: "shortestGeometricPath",
            signature: "shortestGeometricPath(source: number, target: number): any",
            description: "Find shortest path using geometric distances"
          },
          {
            name: "computeGeometricCentrality",
            signature: "computeGeometricCentrality(): Float64Array",
            description: "Centrality based on geometric distances"
          },
          {
            name: "computeBetweennessCentrality",
            signature: "computeBetweennessCentrality(): Float64Array",
            description: "Standard betweenness centrality"
          },
          {
            name: "findCommunities",
            signature: "findCommunities(num_communities: number): Array<any>",
            description: "Detect communities using geometric clustering"
          },
          {
            name: "spectralClustering",
            signature: "spectralClustering(num_clusters: number): Array<any>",
            description: "Perform spectral clustering"
          },
          {
            name: "simulateDiffusion",
            signature: "simulateDiffusion(initial_sources: Uint32Array, max_steps: number, diffusion_rate: number): WasmPropagationAnalysis",
            description: "Simulate information diffusion"
          },
          {
            name: "toTropicalNetwork",
            signature: "toTropicalNetwork(): WasmTropicalNetwork",
            description: "Convert to tropical network"
          }
        ]
      },
      {
        name: "WasmTropicalNetwork",
        description: "Network with tropical algebra for shortest paths",
        methods: [
          {
            name: "constructor",
            signature: "new WasmTropicalNetwork(size: number)",
            description: "Create network with given size"
          },
          {
            name: "fromWeights",
            signature: "static fromWeights(weights: Float64Array, size: number): WasmTropicalNetwork",
            description: "Create from weight matrix",
            isStatic: true
          },
          {
            name: "setEdge",
            signature: "setEdge(source: number, target: number, weight: number): void",
            description: "Set edge weight"
          },
          {
            name: "shortestPathTropical",
            signature: "shortestPathTropical(source: number, target: number): any",
            description: "Find shortest path using tropical algebra"
          },
          {
            name: "tropicalBetweenness",
            signature: "tropicalBetweenness(): Float64Array",
            description: "Tropical betweenness centrality"
          }
        ]
      },
      {
        name: "NetworkUtils",
        description: "Network creation utilities",
        methods: [
          {
            name: "createRandomNetwork",
            signature: "static createRandomNetwork(num_nodes: number, connection_probability: number): WasmGeometricNetwork",
            description: "Create Erdős-Rényi random network",
            isStatic: true
          },
          {
            name: "createSmallWorldNetwork",
            signature: "static createSmallWorldNetwork(num_nodes: number, k: number, beta: number): WasmGeometricNetwork",
            description: "Create Watts-Strogatz small-world network",
            isStatic: true
          },
          {
            name: "clusteringCoefficient",
            signature: "static clusteringCoefficient(network: WasmGeometricNetwork): number",
            description: "Compute global clustering coefficient",
            isStatic: true
          }
        ]
      }
    ]
  },
  {
    id: "relativistic",
    title: "Relativistic Physics",
    description: "Special and general relativity with spacetime vectors and geodesics",
    icon: "⚛",
    classes: [
      {
        name: "WasmSpacetimeVector",
        description: "4-vector (ct, x, y, z) with Minkowski metric",
        methods: [
          {
            name: "constructor",
            signature: "new WasmSpacetimeVector(t: number, x: number, y: number, z: number)",
            description: "Create spacetime vector"
          },
          {
            name: "timelike",
            signature: "static timelike(t: number): WasmSpacetimeVector",
            description: "Create pure timelike vector",
            isStatic: true
          },
          {
            name: "spacelike",
            signature: "static spacelike(x: number, y: number, z: number): WasmSpacetimeVector",
            description: "Create pure spacelike vector",
            isStatic: true
          },
          {
            name: "minkowski_dot",
            signature: "minkowski_dot(other: WasmSpacetimeVector): number",
            description: "Minkowski inner product: t₁t₂ - x₁x₂ - y₁y₂ - z₁z₂"
          },
          {
            name: "norm_squared",
            signature: "norm_squared(): number",
            description: "Minkowski norm squared"
          },
          {
            name: "is_timelike",
            signature: "is_timelike(): boolean",
            description: "Check if timelike (norm² > 0)"
          },
          {
            name: "is_spacelike",
            signature: "is_spacelike(): boolean",
            description: "Check if spacelike (norm² < 0)"
          },
          {
            name: "is_null",
            signature: "is_null(): boolean",
            description: "Check if null/lightlike (norm² = 0)"
          }
        ]
      },
      {
        name: "WasmFourVelocity",
        description: "Relativistic four-velocity",
        methods: [
          {
            name: "from_velocity",
            signature: "static from_velocity(vx: number, vy: number, vz: number): WasmFourVelocity",
            description: "Create from 3-velocity components",
            isStatic: true
          },
          {
            name: "gamma",
            signature: "gamma(): number",
            description: "Lorentz factor γ = 1/√(1-v²/c²)"
          },
          {
            name: "rapidity",
            signature: "rapidity(): number",
            description: "Rapidity φ = arctanh(v/c)"
          },
          {
            name: "spatial_velocity_magnitude",
            signature: "spatial_velocity_magnitude(): number",
            description: "3-velocity magnitude"
          },
          {
            name: "is_normalized",
            signature: "is_normalized(): boolean",
            description: "Check if u·u = c²"
          },
          {
            name: "as_spacetime_vector",
            signature: "as_spacetime_vector(): WasmSpacetimeVector",
            description: "Convert to spacetime vector"
          }
        ]
      },
      {
        name: "WasmRelativisticParticle",
        description: "Relativistic particle with position, velocity, mass, and charge",
        methods: [
          {
            name: "constructor",
            signature: "new WasmRelativisticParticle(x: number, y: number, z: number, vx: number, vy: number, vz: number, spin: number, mass: number, charge: number)",
            description: "Create particle"
          },
          {
            name: "with_energy",
            signature: "static with_energy(x: number, y: number, z: number, direction_x: number, direction_y: number, direction_z: number, kinetic_energy: number, mass: number, charge: number): WasmRelativisticParticle",
            description: "Create with specified kinetic energy",
            isStatic: true
          },
          {
            name: "four_velocity",
            signature: "four_velocity(): WasmFourVelocity",
            description: "Get four-velocity"
          },
          {
            name: "kinetic_energy",
            signature: "kinetic_energy(): number",
            description: "Kinetic energy (γ-1)mc²"
          },
          {
            name: "total_energy",
            signature: "total_energy(): number",
            description: "Total energy γmc²"
          },
          {
            name: "momentum_magnitude",
            signature: "momentum_magnitude(): number",
            description: "3-momentum magnitude γmv"
          }
        ]
      },
      {
        name: "WasmSchwarzschildMetric",
        description: "Schwarzschild spacetime metric for spherically symmetric masses",
        methods: [
          {
            name: "from_mass",
            signature: "static from_mass(mass: number): WasmSchwarzschildMetric",
            description: "Create for custom mass",
            isStatic: true
          },
          {
            name: "sun",
            signature: "static sun(): WasmSchwarzschildMetric",
            description: "Create for Solar mass",
            isStatic: true
          },
          {
            name: "earth",
            signature: "static earth(): WasmSchwarzschildMetric",
            description: "Create for Earth mass",
            isStatic: true
          },
          {
            name: "schwarzschild_radius",
            signature: "schwarzschild_radius(): number",
            description: "Event horizon radius rs = 2GM/c²"
          },
          {
            name: "effective_potential",
            signature: "effective_potential(r: number, angular_momentum: number): number",
            description: "Effective potential for orbital motion"
          },
          {
            name: "has_singularity",
            signature: "has_singularity(position: WasmSpacetimeVector): boolean",
            description: "Check if position is at singularity"
          }
        ]
      },
      {
        name: "WasmRelativisticConstants",
        description: "Physical constants",
        methods: [
          {
            name: "speed_of_light",
            signature: "static readonly speed_of_light: number",
            description: "c = 299,792,458 m/s",
            isStatic: true
          },
          {
            name: "gravitational_constant",
            signature: "static readonly gravitational_constant: number",
            description: "G = 6.674×10⁻¹¹ m³/kg·s²",
            isStatic: true
          },
          {
            name: "solar_mass",
            signature: "static readonly solar_mass: number",
            description: "M☉ = 1.989×10³⁰ kg",
            isStatic: true
          }
        ]
      }
    ],
    functions: [
      {
        name: "velocity_to_gamma",
        signature: "velocity_to_gamma(velocity_magnitude: number): number",
        description: "Convert velocity to Lorentz factor"
      },
      {
        name: "gamma_to_velocity",
        signature: "gamma_to_velocity(gamma: number): number",
        description: "Convert Lorentz factor to velocity"
      },
      {
        name: "light_deflection_angle",
        signature: "light_deflection_angle(impact_parameter: number, mass: number): number",
        description: "Calculate gravitational light bending"
      }
    ]
  },
  {
    id: "optimization",
    title: "Optimization",
    description: "Gradient descent, GPU-accelerated, and multi-objective optimization",
    icon: "⬡",
    classes: [
      {
        name: "WasmSimpleOptimizer",
        description: "Simple gradient-based optimization",
        methods: [
          {
            name: "constructor",
            signature: "new WasmSimpleOptimizer()",
            description: "Create optimizer"
          },
          {
            name: "optimizeQuadratic",
            signature: "optimizeQuadratic(coefficients: Float64Array, initial_point: Float64Array): WasmOptimizationResult",
            description: "Minimize Σ cᵢxᵢ²"
          }
        ]
      },
      {
        name: "WasmGpuOptimizer",
        description: "GPU-accelerated optimization (when available)",
        methods: [
          {
            name: "constructor",
            signature: "new WasmGpuOptimizer()",
            description: "Create GPU optimizer"
          },
          {
            name: "initializeGpu",
            signature: "initializeGpu(): Promise<boolean>",
            description: "Initialize GPU context"
          },
          {
            name: "isGpuAvailable",
            signature: "isGpuAvailable(): boolean",
            description: "Check GPU availability"
          },
          {
            name: "optimizeQuadraticGpu",
            signature: "optimizeQuadraticGpu(coefficients: Float64Array, initial_point: Float64Array, max_iterations: number, tolerance: number): Promise<WasmOptimizationResult>",
            description: "GPU-accelerated quadratic optimization"
          },
          {
            name: "optimizeBatch",
            signature: "optimizeBatch(problems_data: Float64Array, problem_size: number, num_problems: number, max_iterations: number, tolerance: number): Promise<Float64Array>",
            description: "Batch optimization with parallel processing"
          }
        ]
      },
      {
        name: "WasmMultiObjectiveOptimizer",
        description: "Multi-objective Pareto optimization",
        methods: [
          {
            name: "constructor",
            signature: "new WasmMultiObjectiveOptimizer()",
            description: "Create multi-objective optimizer"
          },
          {
            name: "optimizeBiObjective",
            signature: "optimizeBiObjective(dimension: number, population_size: number, generations: number): WasmMultiObjectiveResult",
            description: "Optimize bi-objective problem"
          }
        ]
      },
      {
        name: "WasmOptimizationResult",
        description: "Optimization result",
        methods: [
          {
            name: "solution",
            signature: "readonly solution: Float64Array",
            description: "Optimal solution vector"
          },
          {
            name: "objective_value",
            signature: "readonly objective_value: number",
            description: "Final objective value"
          },
          {
            name: "converged",
            signature: "readonly converged: boolean",
            description: "Whether optimization converged"
          },
          {
            name: "iterations",
            signature: "readonly iterations: number",
            description: "Number of iterations"
          }
        ]
      }
    ]
  },
  {
    id: "holographic",
    title: "Holographic Memory & Optical Fields",
    description: "Distributed memory, VSA operations, and Lee hologram encoding using geometric algebra",
    icon: "⊗",
    classes: [
      {
        name: "WasmHolographicMemory",
        description: "Content-addressable memory using holographic reduced representations",
        methods: [
          {
            name: "constructor",
            signature: "new WasmHolographicMemory()",
            description: "Create holographic memory"
          },
          {
            name: "withKeyTracking",
            signature: "static withKeyTracking(): WasmHolographicMemory",
            description: "Create with key tracking enabled",
            isStatic: true
          },
          {
            name: "randomVersor",
            signature: "static randomVersor(num_factors: number): Float64Array",
            description: "Generate random versor for keys/values",
            isStatic: true
          },
          {
            name: "store",
            signature: "store(key: Float64Array, value: Float64Array): void",
            description: "Store key-value pair (256 coefficients each)"
          },
          {
            name: "retrieve",
            signature: "retrieve(key: Float64Array): Float64Array",
            description: "Retrieve value by key"
          },
          {
            name: "retrieveConfidence",
            signature: "retrieveConfidence(key: Float64Array): number",
            description: "Get retrieval confidence for a key"
          },
          {
            name: "clear",
            signature: "clear(): void",
            description: "Clear all stored items"
          },
          {
            name: "itemCount",
            signature: "itemCount(): number",
            description: "Number of stored items"
          },
          {
            name: "estimatedSnr",
            signature: "estimatedSnr(): number",
            description: "Estimated signal-to-noise ratio"
          },
          {
            name: "theoreticalCapacity",
            signature: "theoreticalCapacity(): number",
            description: "Theoretical storage capacity"
          },
          {
            name: "isNearCapacity",
            signature: "isNearCapacity(): boolean",
            description: "Check if memory is near capacity"
          }
        ]
      },
      {
        name: "WasmResonator",
        description: "Cleanup memory for noisy retrieval",
        methods: [
          {
            name: "constructor",
            signature: "new WasmResonator(codebook_flat: Float64Array)",
            description: "Create from flat codebook array"
          },
          {
            name: "cleanup",
            signature: "cleanup(input: Float64Array): Float64Array",
            description: "Clean up noisy input to find closest codebook item"
          },
          {
            name: "cleanupWithInfo",
            signature: "cleanupWithInfo(input: Float64Array): any",
            description: "Cleanup with metadata"
          },
          {
            name: "codebookSize",
            signature: "codebookSize(): number",
            description: "Number of codebook entries"
          }
        ]
      },
      // Optical Field Operations (v0.15.1)
      {
        name: "WasmOpticalRotorField",
        description: "Optical wavefront as a grid of Cl(2,0) rotors. Each point stores phase (rotor angle) and amplitude.",
        methods: [
          {
            name: "constructor",
            signature: "new WasmOpticalRotorField(phase: Float32Array, amplitude: Float32Array, width: number, height: number)",
            description: "Create from phase and amplitude arrays",
            parameters: [
              { name: "phase", type: "Float32Array", description: "Phase values in radians" },
              { name: "amplitude", type: "Float32Array", description: "Amplitude values" },
              { name: "width", type: "number", description: "Grid width" },
              { name: "height", type: "number", description: "Grid height" }
            ],
            example: `const phases = new Float32Array(64 * 64).fill(0);
const amps = new Float32Array(64 * 64).fill(1.0);
const field = new WasmOpticalRotorField(phases, amps, 64, 64);`
          },
          {
            name: "random",
            signature: "static random(width: number, height: number, seed: bigint): WasmOpticalRotorField",
            description: "Create with random phases (deterministic from seed)",
            isStatic: true,
            example: `const field = WasmOpticalRotorField.random(64, 64, 12345n);`
          },
          {
            name: "uniform",
            signature: "static uniform(phase: number, amplitude: number, width: number, height: number): WasmOpticalRotorField",
            description: "Create uniform field with constant phase and amplitude",
            isStatic: true
          },
          {
            name: "identity",
            signature: "static identity(width: number, height: number): WasmOpticalRotorField",
            description: "Create identity field (phase = 0, amplitude = 1)",
            isStatic: true
          },
          {
            name: "phaseAt",
            signature: "phaseAt(x: number, y: number): number",
            description: "Get phase at point (radians, range [-π, π])"
          },
          {
            name: "amplitudeAt",
            signature: "amplitudeAt(x: number, y: number): number",
            description: "Get amplitude at point"
          },
          {
            name: "getScalars",
            signature: "getScalars(): Float32Array",
            description: "Get all scalar (cos φ) components"
          },
          {
            name: "getBivectors",
            signature: "getBivectors(): Float32Array",
            description: "Get all bivector (sin φ) components"
          },
          {
            name: "getAmplitudes",
            signature: "getAmplitudes(): Float32Array",
            description: "Get all amplitude components"
          },
          {
            name: "totalEnergy",
            signature: "totalEnergy(): number",
            description: "Sum of squared amplitudes"
          },
          {
            name: "normalized",
            signature: "normalized(): WasmOpticalRotorField",
            description: "Create normalized copy (total energy = 1)"
          }
        ]
      },
      {
        name: "WasmBinaryHologram",
        description: "Bit-packed binary pattern for DMD display. Output of Lee encoding.",
        methods: [
          {
            name: "constructor",
            signature: "new WasmBinaryHologram(pattern: Uint8Array, width: number, height: number)",
            description: "Create from boolean array (0 = off, non-zero = on)"
          },
          {
            name: "zeros",
            signature: "static zeros(width: number, height: number): WasmBinaryHologram",
            description: "Create all-zeros hologram",
            isStatic: true
          },
          {
            name: "ones",
            signature: "static ones(width: number, height: number): WasmBinaryHologram",
            description: "Create all-ones hologram",
            isStatic: true
          },
          {
            name: "get",
            signature: "get(x: number, y: number): boolean",
            description: "Get pixel value"
          },
          {
            name: "set",
            signature: "set(x: number, y: number, value: boolean): void",
            description: "Set pixel value"
          },
          {
            name: "toggle",
            signature: "toggle(x: number, y: number): void",
            description: "Toggle pixel at (x, y)"
          },
          {
            name: "asBytes",
            signature: "asBytes(): Uint8Array",
            description: "Get packed binary data (LSB-first, row-major)"
          },
          {
            name: "popcount",
            signature: "popcount(): number",
            description: "Count of 'on' pixels"
          },
          {
            name: "fillFactor",
            signature: "fillFactor(): number",
            description: "Fraction of 'on' pixels (0 to 1)"
          },
          {
            name: "hammingDistance",
            signature: "hammingDistance(other: WasmBinaryHologram): number",
            description: "Number of differing pixels"
          },
          {
            name: "xor",
            signature: "xor(other: WasmBinaryHologram): WasmBinaryHologram",
            description: "XOR two holograms"
          },
          {
            name: "inverted",
            signature: "inverted(): WasmBinaryHologram",
            description: "Create inverted copy"
          }
        ]
      },
      {
        name: "WasmGeometricLeeEncoder",
        description: "Lee hologram encoder using geometric algebra. Encodes optical rotor fields to binary patterns.",
        methods: [
          {
            name: "constructor",
            signature: "new WasmGeometricLeeEncoder(width: number, height: number, frequency: number, angle: number)",
            description: "Create encoder with carrier frequency and angle",
            parameters: [
              { name: "width", type: "number", description: "Grid width" },
              { name: "height", type: "number", description: "Grid height" },
              { name: "frequency", type: "number", description: "Carrier frequency (cycles/pixel)" },
              { name: "angle", type: "number", description: "Carrier angle (radians)" }
            ]
          },
          {
            name: "withFrequency",
            signature: "static withFrequency(width: number, height: number, frequency: number): WasmGeometricLeeEncoder",
            description: "Create with horizontal carrier (angle = 0)",
            isStatic: true,
            example: `const encoder = WasmGeometricLeeEncoder.withFrequency(64, 64, 0.25);`
          },
          {
            name: "encode",
            signature: "encode(field: WasmOpticalRotorField): WasmBinaryHologram",
            description: "Encode rotor field to binary hologram",
            example: `const hologram = encoder.encode(field);
console.log(\`Fill: \${hologram.fillFactor()}\`);`
          },
          {
            name: "modulate",
            signature: "modulate(field: WasmOpticalRotorField): WasmOpticalRotorField",
            description: "Get modulated field (before thresholding)"
          },
          {
            name: "theoreticalEfficiency",
            signature: "theoreticalEfficiency(field: WasmOpticalRotorField): number",
            description: "Compute theoretical diffraction efficiency"
          }
        ]
      },
      {
        name: "WasmOpticalFieldAlgebra",
        description: "VSA operations on optical rotor fields: bind, bundle, similarity, inverse.",
        methods: [
          {
            name: "constructor",
            signature: "new WasmOpticalFieldAlgebra(width: number, height: number)",
            description: "Create algebra for fields of given dimensions",
            example: `const algebra = new WasmOpticalFieldAlgebra(64, 64);`
          },
          {
            name: "identity",
            signature: "identity(): WasmOpticalRotorField",
            description: "Create identity field (phase = 0)"
          },
          {
            name: "random",
            signature: "random(seed: bigint): WasmOpticalRotorField",
            description: "Create random field"
          },
          {
            name: "bind",
            signature: "bind(a: WasmOpticalRotorField, b: WasmOpticalRotorField): WasmOpticalRotorField",
            description: "Rotor product (phase addition) - creates association",
            example: `const bound = algebra.bind(role, filler);
// bound encodes role ⊗ filler`
          },
          {
            name: "unbind",
            signature: "unbind(key: WasmOpticalRotorField, bound: WasmOpticalRotorField): WasmOpticalRotorField",
            description: "Retrieve associated field: key⁻¹ ⊗ bound",
            example: `const retrieved = algebra.unbind(role, bound);
// retrieved ≈ filler`
          },
          {
            name: "bundle",
            signature: "bundle(fields: WasmOpticalRotorField[], weights: Float32Array): WasmOpticalRotorField",
            description: "Weighted superposition of multiple fields"
          },
          {
            name: "bundleUniform",
            signature: "bundleUniform(fields: WasmOpticalRotorField[]): WasmOpticalRotorField",
            description: "Bundle with equal weights (1/n)"
          },
          {
            name: "similarity",
            signature: "similarity(a: WasmOpticalRotorField, b: WasmOpticalRotorField): number",
            description: "Normalized inner product. Range [-1, 1].",
            example: `const sim = algebra.similarity(field1, field2);
console.log(\`Self-similarity: \${algebra.similarity(f, f)}\`); // 1.0`
          },
          {
            name: "inverse",
            signature: "inverse(field: WasmOpticalRotorField): WasmOpticalRotorField",
            description: "Rotor reverse (phase negation)"
          },
          {
            name: "scale",
            signature: "scale(field: WasmOpticalRotorField, factor: number): WasmOpticalRotorField",
            description: "Scale amplitude by factor"
          },
          {
            name: "addPhase",
            signature: "addPhase(field: WasmOpticalRotorField, phase: number): WasmOpticalRotorField",
            description: "Add constant phase to all points"
          },
          {
            name: "meanPhase",
            signature: "meanPhase(field: WasmOpticalRotorField): number",
            description: "Circular mean of phases"
          },
          {
            name: "phaseVariance",
            signature: "phaseVariance(field: WasmOpticalRotorField): number",
            description: "Circular variance of phases"
          }
        ]
      },
      {
        name: "WasmOpticalCodebook",
        description: "Maps symbols to deterministically-generated rotor fields. Enables compact checkpointing.",
        methods: [
          {
            name: "constructor",
            signature: "new WasmOpticalCodebook(width: number, height: number, baseSeed: bigint)",
            description: "Create codebook",
            example: `const codebook = new WasmOpticalCodebook(64, 64, 12345n);`
          },
          {
            name: "register",
            signature: "register(symbol: string): void",
            description: "Register symbol with auto-generated seed"
          },
          {
            name: "registerWithSeed",
            signature: "registerWithSeed(symbol: string, seed: bigint): void",
            description: "Register symbol with specific seed"
          },
          {
            name: "registerAll",
            signature: "registerAll(symbols: string[]): void",
            description: "Register multiple symbols"
          },
          {
            name: "get",
            signature: "get(symbol: string): WasmOpticalRotorField | undefined",
            description: "Get or generate field for symbol"
          },
          {
            name: "generate",
            signature: "generate(symbol: string): WasmOpticalRotorField | undefined",
            description: "Generate field without caching"
          },
          {
            name: "contains",
            signature: "contains(symbol: string): boolean",
            description: "Check if symbol is registered"
          },
          {
            name: "getSeed",
            signature: "getSeed(symbol: string): bigint | undefined",
            description: "Get seed for registered symbol"
          },
          {
            name: "symbols",
            signature: "symbols(): string[]",
            description: "Get all registered symbol names"
          },
          {
            name: "clearCache",
            signature: "clearCache(): void",
            description: "Clear field cache (seeds retained)"
          },
          {
            name: "remove",
            signature: "remove(symbol: string): boolean",
            description: "Remove symbol from codebook"
          }
        ]
      },
      {
        name: "WasmTropicalOpticalAlgebra",
        description: "Tropical (min, +) operations on optical fields for attractor dynamics.",
        methods: [
          {
            name: "constructor",
            signature: "new WasmTropicalOpticalAlgebra(width: number, height: number)",
            description: "Create tropical algebra",
            example: `const tropical = new WasmTropicalOpticalAlgebra(32, 32);`
          },
          {
            name: "tropicalAdd",
            signature: "tropicalAdd(a: WasmOpticalRotorField, b: WasmOpticalRotorField): WasmOpticalRotorField",
            description: "Pointwise minimum phase magnitude"
          },
          {
            name: "tropicalMax",
            signature: "tropicalMax(a: WasmOpticalRotorField, b: WasmOpticalRotorField): WasmOpticalRotorField",
            description: "Pointwise maximum phase magnitude"
          },
          {
            name: "tropicalMul",
            signature: "tropicalMul(a: WasmOpticalRotorField, b: WasmOpticalRotorField): WasmOpticalRotorField",
            description: "Tropical multiplication (binding/phase addition)"
          },
          {
            name: "softTropicalAdd",
            signature: "softTropicalAdd(a: WasmOpticalRotorField, b: WasmOpticalRotorField, beta: number): WasmOpticalRotorField",
            description: "Soft minimum with temperature parameter"
          },
          {
            name: "phaseDistance",
            signature: "phaseDistance(a: WasmOpticalRotorField, b: WasmOpticalRotorField): number",
            description: "Sum of absolute phase differences"
          },
          {
            name: "normalizedPhaseDistance",
            signature: "normalizedPhaseDistance(a: WasmOpticalRotorField, b: WasmOpticalRotorField): number",
            description: "Average phase difference per pixel"
          },
          {
            name: "attractorConverge",
            signature: "attractorConverge(initial: WasmOpticalRotorField, attractors: WasmOpticalRotorField[], maxIter: number, tolerance: number): WasmOpticalRotorField",
            description: "Iterate until convergence to attractor",
            example: `const result = tropical.attractorConverge(
  initial, attractors, 100, 0.001
);`
          }
        ]
      }
    ]
  },
  {
    id: "automata",
    title: "Cellular Automata",
    description: "Geometric cellular automata and self-assembly systems",
    icon: "▦",
    classes: [
      {
        name: "WasmGeometricCA",
        description: "Cellular automaton with multivector cell states",
        methods: [
          {
            name: "constructor",
            signature: "new WasmGeometricCA(width: number, height: number)",
            description: "Create 2D geometric CA"
          },
          {
            name: "step",
            signature: "step(): void",
            description: "Evolve one generation"
          },
          {
            name: "reset",
            signature: "reset(): void",
            description: "Reset to initial state"
          },
          {
            name: "setCell",
            signature: "setCell(x: number, y: number, coefficients: Float64Array): void",
            description: "Set cell value"
          },
          {
            name: "getCell",
            signature: "getCell(x: number, y: number): Float64Array",
            description: "Get cell value"
          },
          {
            name: "getGrid",
            signature: "getGrid(): Float64Array",
            description: "Get entire grid as flat array"
          },
          {
            name: "setGrid",
            signature: "setGrid(grid: Float64Array): void",
            description: "Set entire grid"
          },
          {
            name: "addRandomPattern",
            signature: "addRandomPattern(density: number): void",
            description: "Add random pattern with given density"
          },
          {
            name: "addGlider",
            signature: "addGlider(x: number, y: number): void",
            description: "Add glider pattern"
          },
          {
            name: "setRule",
            signature: "setRule(rule_type: string): void",
            description: "Set CA rule type"
          },
          {
            name: "generation",
            signature: "generation(): number",
            description: "Current generation number"
          },
          {
            name: "getPopulation",
            signature: "getPopulation(): number",
            description: "Count of non-zero cells"
          },
          {
            name: "getTotalEnergy",
            signature: "getTotalEnergy(): number",
            description: "Total energy of system"
          }
        ]
      },
      {
        name: "WasmSelfAssembler",
        description: "Self-assembly simulation",
        methods: [
          {
            name: "constructor",
            signature: "new WasmSelfAssembler()",
            description: "Create self-assembler"
          },
          {
            name: "addComponent",
            signature: "addComponent(type_name: string, position: Float64Array): number",
            description: "Add component to system"
          },
          {
            name: "checkStability",
            signature: "checkStability(): boolean",
            description: "Check if assembly is stable"
          },
          {
            name: "getComponentCount",
            signature: "getComponentCount(): number",
            description: "Number of components"
          }
        ]
      },
      {
        name: "WasmInverseCADesigner",
        description: "Find CA seeds that produce target patterns",
        methods: [
          {
            name: "constructor",
            signature: "new WasmInverseCADesigner(target_width: number, target_height: number)",
            description: "Create inverse designer"
          },
          {
            name: "setTarget",
            signature: "setTarget(target_grid: Float64Array): void",
            description: "Set target pattern"
          },
          {
            name: "findSeed",
            signature: "findSeed(max_generations: number, max_attempts: number): Float64Array",
            description: "Find seed that produces target"
          },
          {
            name: "evaluateFitness",
            signature: "evaluateFitness(candidate: Float64Array): number",
            description: "Evaluate fitness of candidate seed"
          }
        ]
      },
      {
        name: "AutomataUtils",
        description: "Automata utilities",
        methods: [
          {
            name: "validateGrid",
            signature: "static validateGrid(grid: Float64Array, width: number, height: number): boolean",
            description: "Validate grid dimensions",
            isStatic: true
          },
          {
            name: "createLifePattern",
            signature: "static createLifePattern(pattern_name: string, width: number, height: number): Float64Array",
            description: "Create standard Game of Life pattern",
            isStatic: true
          },
          {
            name: "generateRandomSeed",
            signature: "static generateRandomSeed(width: number, height: number, density: number): Float64Array",
            description: "Generate random seed",
            isStatic: true
          }
        ]
      }
    ]
  },
  {
    id: "enumerative",
    title: "Enumerative Geometry",
    description: "Intersection theory, Schubert calculus, LR coefficients, namespaces, curve counting, matroids, stability, and equivariant localization",
    icon: "∩",
    classes: [
      {
        name: "WasmProjectiveSpace",
        description: "Projective space ℙⁿ",
        methods: [
          {
            name: "constructor",
            signature: "new WasmProjectiveSpace(dimension: number)",
            description: "Create projective space of given dimension"
          },
          {
            name: "getDimension",
            signature: "getDimension(): number",
            description: "Get dimension"
          },
          {
            name: "bezoutIntersection",
            signature: "bezoutIntersection(degree1: number, degree2: number): number",
            description: "Compute Bézout intersection number"
          }
        ]
      },
      {
        name: "WasmGrassmannian",
        description: "Grassmannian Gr(k,n) of k-planes in n-space",
        methods: [
          {
            name: "constructor",
            signature: "new WasmGrassmannian(k: number, n: number)",
            description: "Create Gr(k,n)"
          },
          {
            name: "getDimension",
            signature: "getDimension(): number",
            description: "Get dimension k(n-k)"
          },
          {
            name: "getParameters",
            signature: "getParameters(): Uint32Array",
            description: "Get (k, n)"
          }
        ]
      },
      {
        name: "WasmChowClass",
        description: "Chow class in intersection theory",
        methods: [
          {
            name: "constructor",
            signature: "new WasmChowClass(dimension: number, degree: number)",
            description: "Create Chow class"
          },
          {
            name: "point",
            signature: "static point(): WasmChowClass",
            description: "Point class",
            isStatic: true
          },
          {
            name: "linearSubspace",
            signature: "static linearSubspace(codimension: number): WasmChowClass",
            description: "Linear subspace class",
            isStatic: true
          },
          {
            name: "hypersurface",
            signature: "static hypersurface(degree: number): WasmChowClass",
            description: "Hypersurface class of given degree",
            isStatic: true
          },
          {
            name: "multiply",
            signature: "multiply(other: WasmChowClass): WasmChowClass",
            description: "Intersection product"
          },
          {
            name: "power",
            signature: "power(n: number): WasmChowClass",
            description: "Self-intersection"
          }
        ]
      },
      {
        name: "WasmModuliSpace",
        description: "Moduli space of curves M_{g,n}",
        methods: [
          {
            name: "ofCurves",
            signature: "static ofCurves(genus: number, marked_points: number): WasmModuliSpace",
            description: "Create M_{g,n}",
            isStatic: true
          },
          {
            name: "ofStableCurves",
            signature: "static ofStableCurves(genus: number, marked_points: number): WasmModuliSpace",
            description: "Create \\bar{M}_{g,n}",
            isStatic: true
          },
          {
            name: "expectedDimension",
            signature: "expectedDimension(): number",
            description: "Expected dimension 3g-3+n"
          },
          {
            name: "isProper",
            signature: "isProper(): boolean",
            description: "Check if proper (compactified)"
          }
        ]
      },
      {
        name: "EnumerativeUtils",
        description: "Enumerative geometry utilities",
        methods: [
          {
            name: "binomial",
            signature: "static binomial(n: number, k: number): number",
            description: "Binomial coefficient C(n,k)",
            isStatic: true
          },
          {
            name: "bezoutMultiplicity",
            signature: "static bezoutMultiplicity(degree1: number, degree2: number, space_dimension: number): number",
            description: "Bézout intersection multiplicity",
            isStatic: true
          },
          {
            name: "eulerCharacteristic",
            signature: "static eulerCharacteristic(dimension: number): number",
            description: "Euler characteristic of projective space",
            isStatic: true
          },
          {
            name: "expectedRationalCurves",
            signature: "static expectedRationalCurves(degree: number, points: number): number",
            description: "Expected number of rational curves through points",
            isStatic: true
          }
        ]
      },
      {
        name: "WasmPartition",
        description: "A partition (weakly decreasing sequence of positive integers) used in Schubert calculus and Littlewood-Richardson coefficient computation",
        methods: [
          {
            name: "constructor",
            signature: "new WasmPartition(parts: number[])",
            description: "Create a partition from an array of parts",
            parameters: [{ name: "parts", type: "number[]", description: "Weakly decreasing array of positive integers" }],
            example: `const partition = new WasmPartition([3, 2, 1]);
console.log(partition.getSize()); // 6`
          },
          {
            name: "getParts",
            signature: "getParts(): number[]",
            description: "Get the parts of this partition"
          },
          {
            name: "getSize",
            signature: "getSize(): number",
            description: "Get the size (sum of parts)"
          },
          {
            name: "getLength",
            signature: "getLength(): number",
            description: "Get the length (number of non-zero parts)"
          },
          {
            name: "contains",
            signature: "contains(other: WasmPartition): boolean",
            description: "Check if this partition contains another (for skew shapes)"
          },
          {
            name: "isValid",
            signature: "isValid(): boolean",
            description: "Check if partition is valid (weakly decreasing, positive parts)"
          },
          {
            name: "fitsInBox",
            signature: "fitsInBox(k: number, n: number): boolean",
            description: "Check if partition fits in k × (n-k) box for Grassmannian Gr(k,n)"
          }
        ]
      },
      {
        name: "WasmSchubertClass",
        description: "A Schubert class on a Grassmannian Gr(k,n), represented by a partition fitting in a k × (n-k) box",
        methods: [
          {
            name: "constructor",
            signature: "new WasmSchubertClass(partition: number[], k: number, n: number)",
            description: "Create a Schubert class from a partition on Gr(k,n)",
            parameters: [
              { name: "partition", type: "number[]", description: "Partition defining the Schubert variety" },
              { name: "k", type: "number", description: "k-planes in n-space" },
              { name: "n", type: "number", description: "Ambient dimension" }
            ],
            example: `// σ_{2,1} on Gr(2,5)
const class = new WasmSchubertClass([2, 1], 2, 5);`
          },
          {
            name: "getPartition",
            signature: "getPartition(): number[]",
            description: "Get the partition defining this Schubert class"
          },
          {
            name: "getGrassmannianDim",
            signature: "getGrassmannianDim(): number[]",
            description: "Get [k, n] parameters of the Grassmannian"
          },
          {
            name: "getCodimension",
            signature: "getCodimension(): number",
            description: "Get the codimension (sum of partition parts)"
          },
          {
            name: "sigma1",
            signature: "static sigma1(k: number, n: number): WasmSchubertClass",
            description: "Create the special Schubert class σ_1 (single box)",
            isStatic: true
          }
        ]
      },
      {
        name: "WasmSchubertCalculus",
        description: "Schubert calculus computations on Grassmannians. Computes intersection numbers of Schubert cycles.",
        methods: [
          {
            name: "constructor",
            signature: "new WasmSchubertCalculus(k: number, n: number)",
            description: "Create a Schubert calculus context for Gr(k,n)",
            example: `// Lines in P^3 = Gr(2,4)
const calc = new WasmSchubertCalculus(2, 4);`
          },
          {
            name: "intersect",
            signature: "intersect(class1: WasmSchubertClass, class2: WasmSchubertClass): WasmIntersectionResult",
            description: "Compute intersection of two Schubert classes"
          },
          {
            name: "multiIntersect",
            signature: "multiIntersect(classes: WasmSchubertClass[]): WasmIntersectionResult",
            description: "Compute intersection of multiple Schubert classes",
            example: `// How many lines meet 4 general lines in P^3?
const calc = new WasmSchubertCalculus(2, 4);
const sigma1 = WasmSchubertClass.sigma1(2, 4);
const result = calc.multiIntersect([sigma1, sigma1, sigma1, sigma1]);
console.log(result.getCount()); // 2`
          },
          {
            name: "getGrassmannianDimension",
            signature: "getGrassmannianDimension(): number",
            description: "Get dimension k*(n-k) of the Grassmannian"
          }
        ]
      },
      {
        name: "WasmIntersectionResult",
        description: "Result of a Schubert intersection computation. Can be finite (a number), empty, or positive-dimensional.",
        methods: [
          {
            name: "isFinite",
            signature: "isFinite(): boolean",
            description: "Check if intersection is finite (0-dimensional)"
          },
          {
            name: "isEmpty",
            signature: "isEmpty(): boolean",
            description: "Check if intersection is empty"
          },
          {
            name: "isPositiveDimensional",
            signature: "isPositiveDimensional(): boolean",
            description: "Check if intersection has positive dimension"
          },
          {
            name: "getCount",
            signature: "getCount(): number",
            description: "Get finite intersection count (0 if not finite)"
          },
          {
            name: "getDimension",
            signature: "getDimension(): number",
            description: "Get dimension (-1 if empty, 0 if finite)"
          },
          {
            name: "asString",
            signature: "asString(): string",
            description: "Get string representation"
          }
        ]
      },
      {
        name: "WasmCapability",
        description: "A capability (access condition) represented as a Schubert class. Used for geometric access control.",
        methods: [
          {
            name: "constructor",
            signature: "new WasmCapability(id: string, name: string, partition: number[], k: number, n: number)",
            description: "Create a capability with Schubert condition",
            parameters: [
              { name: "id", type: "string", description: "Unique capability identifier" },
              { name: "name", type: "string", description: "Human-readable name" },
              { name: "partition", type: "number[]", description: "Schubert condition as partition" },
              { name: "k", type: "number", description: "Grassmannian k parameter" },
              { name: "n", type: "number", description: "Grassmannian n parameter" }
            ],
            example: `const readCap = new WasmCapability("read", "Read Access", [1], 2, 4);`
          },
          {
            name: "getId",
            signature: "getId(): string",
            description: "Get capability ID"
          },
          {
            name: "getName",
            signature: "getName(): string",
            description: "Get capability name"
          },
          {
            name: "getCodimension",
            signature: "getCodimension(): number",
            description: "Get codimension of capability's Schubert condition"
          },
          {
            name: "requires",
            signature: "requires(requiredId: string): WasmCapability",
            description: "Add a dependency on another capability"
          }
        ]
      },
      {
        name: "WasmNamespace",
        description: "A namespace for geometric access control. Represents a position in a Grassmannian restricted by granted capabilities.",
        methods: [
          {
            name: "full",
            signature: "static full(name: string, k: number, n: number): WasmNamespace",
            description: "Create a full namespace (identity position) on Gr(k,n)",
            isStatic: true,
            example: `const ns = WasmNamespace.full("agent", 2, 4);
console.log(ns.getRemainingDimension()); // 4`
          },
          {
            name: "getName",
            signature: "getName(): string",
            description: "Get namespace name"
          },
          {
            name: "getGrassmannian",
            signature: "getGrassmannian(): number[]",
            description: "Get [k, n] Grassmannian parameters"
          },
          {
            name: "getCapabilityCount",
            signature: "getCapabilityCount(): number",
            description: "Get number of granted capabilities"
          },
          {
            name: "grant",
            signature: "grant(capability: WasmCapability): void",
            description: "Grant a capability to this namespace"
          },
          {
            name: "countConfigurations",
            signature: "countConfigurations(): WasmIntersectionResult",
            description: "Count valid configurations (Schubert intersection number)"
          },
          {
            name: "getTotalCodimension",
            signature: "getTotalCodimension(): number",
            description: "Get total codimension from all capabilities"
          },
          {
            name: "getRemainingDimension",
            signature: "getRemainingDimension(): number",
            description: "Get remaining dimension after capabilities"
          }
        ]
      },
      {
        name: "WasmNamespaceIntersection",
        description: "Result of intersecting two namespaces. Can be incompatible, disjoint, single point, finite points, or a subspace.",
        methods: [
          {
            name: "isIncompatible",
            signature: "isIncompatible(): boolean",
            description: "Check if namespaces are on different Grassmannians"
          },
          {
            name: "isDisjoint",
            signature: "isDisjoint(): boolean",
            description: "Check if intersection is empty"
          },
          {
            name: "isSinglePoint",
            signature: "isSinglePoint(): boolean",
            description: "Check if intersection is exactly one point"
          },
          {
            name: "isFinitePoints",
            signature: "isFinitePoints(): boolean",
            description: "Check if intersection is finite (multiple points)"
          },
          {
            name: "isSubspace",
            signature: "isSubspace(): boolean",
            description: "Check if intersection is a positive-dimensional subspace"
          },
          {
            name: "getCount",
            signature: "getCount(): number",
            description: "Get intersection point count"
          },
          {
            name: "getDimension",
            signature: "getDimension(): number",
            description: "Get dimension (-1 if not a subspace)"
          },
          {
            name: "asString",
            signature: "asString(): string",
            description: "Get string representation"
          }
        ]
      },
      {
        name: "SchubertBatch",
        description: "Batch operations for high-performance Schubert calculus computations",
        methods: [
          {
            name: "multiIntersectBatch",
            signature: "static multiIntersectBatch(classBatches: number[][][], k: number, n: number): WasmIntersectionResult[]",
            description: "Compute multiple Schubert multi-intersections in batch",
            isStatic: true
          },
          {
            name: "linesMeeting4Lines",
            signature: "static linesMeeting4Lines(): WasmIntersectionResult",
            description: "Compute the classic 'lines meeting 4 lines' problem (σ_1^4 in Gr(2,4) = 2)",
            isStatic: true,
            example: `const result = SchubertBatch.linesMeeting4Lines();
console.log(result.getCount()); // 2`
          }
        ]
      },
      {
        name: "NamespaceBatch",
        description: "Batch operations for namespace computations",
        methods: [
          {
            name: "countConfigurationsBatch",
            signature: "static countConfigurationsBatch(namespaces: WasmNamespace[]): WasmIntersectionResult[]",
            description: "Count configurations for multiple namespaces in batch",
            isStatic: true
          }
        ]
      },
      {
        name: "LR Functions",
        description: "Standalone functions for Littlewood-Richardson coefficient computation",
        methods: [
          {
            name: "lrCoefficient",
            signature: "lrCoefficient(lambda: WasmPartition, mu: WasmPartition, nu: WasmPartition): number",
            description: "Compute Littlewood-Richardson coefficient c^ν_{λμ}",
            example: `const lambda = new WasmPartition([2, 1]);
const mu = new WasmPartition([1, 1]);
const nu = new WasmPartition([3, 2]);
const coeff = lrCoefficient(lambda, mu, nu);`
          },
          {
            name: "lrCoefficientsBatch",
            signature: "lrCoefficientsBatch(lambdas: WasmPartition[], mus: WasmPartition[], nus: WasmPartition[]): number[]",
            description: "Compute multiple LR coefficients in batch"
          },
          {
            name: "schubertProduct",
            signature: "schubertProduct(lambda: WasmPartition, mu: WasmPartition, k: number, n: number): Array<{partition: number[], coefficient: number}>",
            description: "Expand Schubert product σ_λ · σ_μ into sum of Schubert classes"
          },
          {
            name: "capabilityAccessible",
            signature: "capabilityAccessible(namespace: WasmNamespace, capability: WasmCapability): boolean",
            description: "Check if a capability is accessible in a namespace"
          },
          {
            name: "namespaceIntersection",
            signature: "namespaceIntersection(ns1: WasmNamespace, ns2: WasmNamespace): WasmNamespaceIntersection",
            description: "Compute the intersection of two namespaces"
          }
        ]
      },
      {
        name: "WasmWDVVEngine",
        description: "Engine for computing genus-0 Gromov-Witten invariants N_d via Kontsevich's recursion (WDVV equations). Counts rational degree-d curves in P² through 3d-1 general points.",
        methods: [
          {
            name: "constructor",
            signature: "new WasmWDVVEngine()",
            description: "Create a new WDVV engine with base cases N_1 = N_2 = 1 seeded",
            example: `const engine = new WasmWDVVEngine();
console.log(engine.rationalCurveCount(3)); // 12`
          },
          {
            name: "rationalCurveCount",
            signature: "rationalCurveCount(degree: number): number",
            description: "Compute N_d: number of rational degree-d curves in P² through 3d-1 general points. Returns result as f64 (JS number). Known values: N_1=1, N_2=1, N_3=12, N_4=620, N_5=87304."
          },
          {
            name: "gwInvariantRational",
            signature: "gwInvariantRational(degree: number): number",
            description: "Compute the Gromov-Witten invariant ⟨H², ..., H²⟩_{0,d} for P² (same as rationalCurveCount)"
          },
          {
            name: "requiredPointCount",
            signature: "static requiredPointCount(degree: number, genus: number): number",
            description: "Number of marked points for the dimension constraint: 3d + g - 1",
            isStatic: true
          },
          {
            name: "getTable",
            signature: "getTable(): Array<{degree: number, count: number}>",
            description: "Return all computed N_d values as a sorted table"
          },
          {
            name: "p1xp1Count",
            signature: "static p1xp1Count(a: number, b: number): number",
            description: "Count rational curves on P¹×P¹ of bidegree (a,b)",
            isStatic: true
          },
          {
            name: "p3Count",
            signature: "static p3Count(degree: number): number",
            description: "Count rational curves in P³ of degree d meeting 4d general lines",
            isStatic: true
          }
        ]
      },
      {
        name: "WasmTorusWeights",
        description: "Torus weights for equivariant localization computations. Represents the weights of a torus action on the ambient space.",
        methods: [
          {
            name: "standard",
            signature: "static standard(n: number): WasmTorusWeights",
            description: "Create standard weights (1, 2, ..., n)",
            isStatic: true
          },
          {
            name: "constructor",
            signature: "new WasmTorusWeights(weights: Float64Array)",
            description: "Create custom torus weights from an array of values"
          },
          {
            name: "getWeights",
            signature: "getWeights(): Float64Array",
            description: "Get the weight values"
          }
        ]
      },
      {
        name: "WasmFixedPoint",
        description: "A torus-fixed point on a Grassmannian Gr(k,n), represented by a k-element subset of {0,...,n-1}.",
        methods: [
          {
            name: "constructor",
            signature: "new WasmFixedPoint(subset: Uint32Array, k: number, n: number)",
            description: "Create a fixed point from a k-element subset",
            example: `const fp = new WasmFixedPoint(new Uint32Array([0, 1]), 2, 4);
console.log(fp.toPartition()); // partition corresponding to this fixed point`
          },
          {
            name: "getSubset",
            signature: "getSubset(): Uint32Array",
            description: "Get the subset defining this fixed point"
          },
          {
            name: "getGrassmannian",
            signature: "getGrassmannian(): Uint32Array",
            description: "Get [k, n] parameters"
          },
          {
            name: "tangentEulerClass",
            signature: "tangentEulerClass(weights: WasmTorusWeights): number",
            description: "Compute the Euler class of the tangent space at this fixed point: ∏(t_j - t_i) for i in subset, j not in subset"
          },
          {
            name: "toPartition",
            signature: "toPartition(): Uint32Array",
            description: "Convert fixed point to the corresponding Schubert partition"
          }
        ]
      },
      {
        name: "WasmEquivariantLocalizer",
        description: "Equivariant localization engine for Grassmannians. Uses the Atiyah-Bott fixed point formula to compute integrals over Gr(k,n).",
        methods: [
          {
            name: "constructor",
            signature: "new WasmEquivariantLocalizer(k: number, n: number)",
            description: "Create a localizer for Gr(k,n) with standard torus weights",
            example: `// Verify σ_1^4 = 2 on Gr(2,4) via localization
const loc = new WasmEquivariantLocalizer(2, 4);
const classes = [new Uint32Array([1]), new Uint32Array([1]),
                 new Uint32Array([1]), new Uint32Array([1])];
console.log(loc.localizedIntersection(classes)); // 2.0`
          },
          {
            name: "withWeights",
            signature: "withWeights(weights: WasmTorusWeights): WasmEquivariantLocalizer",
            description: "Create a localizer with custom torus weights"
          },
          {
            name: "fixedPointCount",
            signature: "fixedPointCount(): number",
            description: "Number of T-fixed points on Gr(k,n) = C(n,k)"
          },
          {
            name: "localizedIntersection",
            signature: "localizedIntersection(classes: Uint32Array[]): number",
            description: "Compute intersection number of Schubert classes via localization formula"
          }
        ]
      },
      {
        name: "WasmMatroid",
        description: "A matroid represented by its bases. Supports uniform matroids, Schubert matroids, duality, deletion, contraction, and direct sum.",
        methods: [
          {
            name: "uniform",
            signature: "static uniform(k: number, n: number): WasmMatroid",
            description: "Create the uniform matroid U_{k,n} where every k-element subset is a basis",
            isStatic: true,
            example: `const m = WasmMatroid.uniform(2, 4);
console.log(m.getRank());      // 2
console.log(m.getNumBases());  // 6`
          },
          {
            name: "schubertMatroid",
            signature: "static schubertMatroid(partition: Uint32Array, k: number, n: number): WasmMatroid",
            description: "Create a Schubert matroid from a partition on Gr(k,n)",
            isStatic: true
          },
          {
            name: "getRank",
            signature: "getRank(): number",
            description: "Get the rank of the matroid"
          },
          {
            name: "getGroundSetSize",
            signature: "getGroundSetSize(): number",
            description: "Get the ground set size"
          },
          {
            name: "getNumBases",
            signature: "getNumBases(): number",
            description: "Get the number of bases"
          },
          {
            name: "rankOf",
            signature: "rankOf(subset: Uint32Array): number",
            description: "Compute the rank of a subset"
          },
          {
            name: "isLoop",
            signature: "isLoop(element: number): boolean",
            description: "Check if element is a loop (rank 0 element)"
          },
          {
            name: "isColoop",
            signature: "isColoop(element: number): boolean",
            description: "Check if element is a coloop (in every basis)"
          },
          {
            name: "dual",
            signature: "dual(): WasmMatroid",
            description: "Compute the dual matroid (bases are complements of original bases)"
          },
          {
            name: "deleteElement",
            signature: "deleteElement(element: number): WasmMatroid",
            description: "Delete an element from the matroid"
          },
          {
            name: "contractElement",
            signature: "contractElement(element: number): WasmMatroid",
            description: "Contract an element in the matroid"
          },
          {
            name: "directSum",
            signature: "directSum(other: WasmMatroid): WasmMatroid",
            description: "Compute the direct sum of two matroids"
          },
          {
            name: "intersectionCardinality",
            signature: "intersectionCardinality(other: WasmMatroid): number",
            description: "Compute the intersection cardinality (max common independent set size)"
          }
        ]
      },
      {
        name: "WasmCSMClass",
        description: "Chern-Schwartz-MacPherson (CSM) class of a Schubert variety or cell. Captures singularity information via characteristic classes.",
        methods: [
          {
            name: "ofSchubertCell",
            signature: "static ofSchubertCell(partition: Uint32Array, k: number, n: number): WasmCSMClass",
            description: "Compute CSM class of a Schubert cell (open stratum)",
            isStatic: true,
            example: `const csm = WasmCSMClass.ofSchubertCell(new Uint32Array([1]), 2, 4);
console.log(csm.eulerCharacteristic()); // 1 (cells are smooth)`
          },
          {
            name: "ofSchubertVariety",
            signature: "static ofSchubertVariety(partition: Uint32Array, k: number, n: number): WasmCSMClass",
            description: "Compute CSM class of a Schubert variety (closed, possibly singular)",
            isStatic: true
          },
          {
            name: "eulerCharacteristic",
            signature: "eulerCharacteristic(): number",
            description: "Get the Euler characteristic (degree of CSM class)"
          },
          {
            name: "intersect",
            signature: "intersect(other: WasmCSMClass): WasmCSMClass",
            description: "Intersect two CSM classes"
          }
        ]
      },
      {
        name: "WasmComposableNamespace",
        description: "A namespace with marked input/output interfaces for operadic composition. Used to compose two namespaces along matching interfaces.",
        methods: [
          {
            name: "constructor",
            signature: "new WasmComposableNamespace(namespace: WasmNamespace)",
            description: "Create a composable namespace wrapper around an existing namespace"
          },
          {
            name: "markOutput",
            signature: "markOutput(capabilityId: string): void",
            description: "Mark a capability as an output interface"
          },
          {
            name: "markInput",
            signature: "markInput(capabilityId: string): void",
            description: "Mark a capability as an input interface"
          },
          {
            name: "outputCount",
            signature: "outputCount(): number",
            description: "Get number of output interfaces"
          },
          {
            name: "inputCount",
            signature: "inputCount(): number",
            description: "Get number of input interfaces"
          },
          {
            name: "effectiveCapabilityCount",
            signature: "effectiveCapabilityCount(): number",
            description: "Get effective capability count (total minus interface caps)"
          }
        ]
      },
      {
        name: "WasmStabilityCondition",
        description: "A Bridgeland-type stability condition on the derived category of Gr(k,n). Assigns phases to objects based on a trust parameter.",
        methods: [
          {
            name: "constructor",
            signature: "new WasmStabilityCondition(k: number, n: number, trustLevel: number)",
            description: "Create a stability condition on Gr(k,n) with given trust level",
            example: `const cond = new WasmStabilityCondition(2, 4, 0.8);
const phase = cond.phase(schubertClass); // phase in (0, 1]`
          },
          {
            name: "phase",
            signature: "phase(class: WasmSchubertClass): number",
            description: "Compute the phase of a Schubert class under this stability condition"
          },
          {
            name: "isStable",
            signature: "isStable(capability: WasmCapability): boolean",
            description: "Check if a capability is stable under this condition"
          },
          {
            name: "stableCount",
            signature: "stableCount(namespace: WasmNamespace): number",
            description: "Count stable objects in a namespace"
          },
          {
            name: "getTrustLevel",
            signature: "getTrustLevel(): number",
            description: "Get the trust level parameter"
          }
        ]
      },
      {
        name: "WasmWallCrossingEngine",
        description: "Engine for computing wall-crossing phenomena. Finds walls in the stability parameter space where stable counts change.",
        methods: [
          {
            name: "constructor",
            signature: "new WasmWallCrossingEngine(k: number, n: number)",
            description: "Create a wall-crossing engine for Gr(k,n)",
            example: `const engine = new WasmWallCrossingEngine(2, 4);
const walls = engine.computeWalls(namespace);
walls.forEach(w => console.log(\`Wall at t=\${w.trustLevel}\`));`
          },
          {
            name: "computeWalls",
            signature: "computeWalls(namespace: WasmNamespace): Array<{trustLevel: number, direction: number, countChange: number}>",
            description: "Find all walls (critical trust values) for a given namespace"
          },
          {
            name: "stableCountAt",
            signature: "stableCountAt(namespace: WasmNamespace, trust: number): number",
            description: "Compute stable count at a specific trust level"
          },
          {
            name: "phaseDiagram",
            signature: "phaseDiagram(namespace: WasmNamespace): Array<{trustLevel: number, stableCount: number}>",
            description: "Generate a phase diagram showing stable count vs trust level"
          }
        ]
      },
      {
        name: "Operad & Composition Functions",
        description: "Functions for operadic composition of namespaces. Compose two namespaces along matching input/output interfaces.",
        methods: [
          {
            name: "composeNamespaces",
            signature: "composeNamespaces(outer: WasmComposableNamespace, inner: WasmComposableNamespace): WasmNamespace",
            description: "Compose two namespaces along matching interfaces (outer's inputs meet inner's outputs)",
            example: `const outer = new WasmComposableNamespace(ns1);
outer.markInput("read");
const inner = new WasmComposableNamespace(ns2);
inner.markOutput("read");
const composed = composeNamespaces(outer, inner);`
          },
          {
            name: "compositionMultiplicity",
            signature: "compositionMultiplicity(outer: WasmComposableNamespace, inner: WasmComposableNamespace): number",
            description: "Compute the multiplicity of a composition (intersection number of interface Schubert classes)"
          },
          {
            name: "interfacesCompatible",
            signature: "interfacesCompatible(outer: WasmComposableNamespace, inner: WasmComposableNamespace): boolean",
            description: "Check if two namespaces have compatible interfaces for composition"
          }
        ]
      }
    ]
  },
  {
    id: "topology",
    title: "Computational Topology",
    description: "Simplicial complexes, homology, persistent homology, and Morse theory",
    icon: "△",
    classes: [
      {
        name: "WasmSimplex",
        description: "An oriented simplex defined by a set of vertices. Simplices are the building blocks of simplicial complexes.",
        methods: [
          {
            name: "constructor",
            signature: "new WasmSimplex(vertices: Uint32Array)",
            description: "Create a simplex from an array of vertex indices",
            parameters: [{ name: "vertices", type: "Uint32Array", description: "Sorted array of vertex indices" }],
            returns: "WasmSimplex",
            example: `// Create a triangle (2-simplex)
const triangle = new WasmSimplex(new Uint32Array([0, 1, 2]));
console.log(triangle.dimension()); // 2`
          },
          {
            name: "dimension",
            signature: "dimension(): number",
            description: "Get the dimension of the simplex (number of vertices - 1)",
            returns: "Dimension (0 for vertex, 1 for edge, 2 for triangle, etc.)"
          },
          {
            name: "vertices",
            signature: "vertices(): Uint32Array",
            description: "Get the vertex indices of this simplex",
            returns: "Uint32Array of vertex indices"
          },
          {
            name: "orientation",
            signature: "orientation(): number",
            description: "Get the orientation (+1 or -1)",
            returns: "Orientation sign"
          },
          {
            name: "faces",
            signature: "faces(): Array<WasmSimplex>",
            description: "Get all (n-1)-dimensional faces of this n-simplex",
            returns: "Array of face simplices"
          },
          {
            name: "boundaryFaces",
            signature: "boundaryFaces(): Array<WasmBoundaryFace>",
            description: "Get boundary faces with their oriented coefficients for the boundary operator",
            returns: "Array of {face: WasmSimplex, sign: number}"
          }
        ]
      },
      {
        name: "WasmSimplicialComplex",
        description: "A simplicial complex: a collection of simplices closed under taking faces. Used for computing homology and topological invariants.",
        methods: [
          {
            name: "constructor",
            signature: "new WasmSimplicialComplex()",
            description: "Create an empty simplicial complex",
            returns: "WasmSimplicialComplex",
            example: `const complex = new WasmSimplicialComplex();
complex.addSimplex(new WasmSimplex(new Uint32Array([0, 1, 2])));`
          },
          {
            name: "addSimplex",
            signature: "addSimplex(simplex: WasmSimplex): void",
            description: "Add a simplex to the complex. Automatically adds all faces.",
            parameters: [{ name: "simplex", type: "WasmSimplex", description: "Simplex to add" }]
          },
          {
            name: "dimension",
            signature: "dimension(): number",
            description: "Get the maximum dimension of any simplex in the complex",
            returns: "Maximum dimension"
          },
          {
            name: "eulerCharacteristic",
            signature: "eulerCharacteristic(): number",
            description: "Compute the Euler characteristic χ = Σ(-1)^k·c_k",
            returns: "Euler characteristic",
            example: `// Tetrahedron surface: χ = 4 - 6 + 4 = 2
console.log(tetrahedron.eulerCharacteristic()); // 2`
          },
          {
            name: "bettiNumbers",
            signature: "bettiNumbers(): Uint32Array",
            description: "Compute the Betti numbers β_k = rank(H_k)",
            returns: "Array of Betti numbers [β₀, β₁, β₂, ...]",
            example: `// Torus: β₀=1, β₁=2, β₂=1
const betti = torus.bettiNumbers();`
          },
          {
            name: "simplexCount",
            signature: "simplexCount(dimension: number): number",
            description: "Count simplices of a given dimension",
            parameters: [{ name: "dimension", type: "number", description: "Dimension to count" }],
            returns: "Number of simplices"
          }
        ]
      },
      {
        name: "WasmFiltration",
        description: "A filtration: a sequence of nested simplicial complexes indexed by a parameter (e.g., distance threshold). Used for persistent homology.",
        methods: [
          {
            name: "constructor",
            signature: "new WasmFiltration()",
            description: "Create an empty filtration",
            returns: "WasmFiltration"
          },
          {
            name: "add",
            signature: "add(simplex: WasmSimplex, time: number): void",
            description: "Add a simplex at a given filtration time",
            parameters: [
              { name: "simplex", type: "WasmSimplex", description: "Simplex to add" },
              { name: "time", type: "number", description: "Filtration parameter value" }
            ]
          },
          {
            name: "complexAt",
            signature: "complexAt(time: number): WasmSimplicialComplex",
            description: "Get the simplicial complex at a given filtration time",
            parameters: [{ name: "time", type: "number", description: "Filtration parameter" }],
            returns: "Simplicial complex containing all simplices with time ≤ t"
          },
          {
            name: "bettiAt",
            signature: "bettiAt(time: number): Uint32Array",
            description: "Compute Betti numbers at a given filtration time",
            parameters: [{ name: "time", type: "number", description: "Filtration parameter" }],
            returns: "Betti numbers at that time"
          },
          {
            name: "length",
            signature: "length(): number",
            description: "Get the number of simplices in the filtration",
            returns: "Number of simplices"
          }
        ]
      },
      {
        name: "WasmPersistentHomology",
        description: "Compute persistent homology of a filtration. Tracks birth and death of topological features across scales.",
        methods: [
          {
            name: "constructor",
            signature: "new WasmPersistentHomology()",
            description: "Create a persistent homology computer",
            returns: "WasmPersistentHomology"
          },
          {
            name: "compute",
            signature: "compute(filtration: WasmFiltration): WasmPersistenceDiagram",
            description: "Compute persistence diagram from a filtration",
            parameters: [{ name: "filtration", type: "WasmFiltration", description: "Input filtration" }],
            returns: "Persistence diagram with birth-death pairs",
            example: `const ph = new WasmPersistentHomology();
const diagram = ph.compute(ripsFiltration);
for (const interval of diagram.getIntervals()) {
  console.log(\`H\${interval.dimension}: [\${interval.birth}, \${interval.death})\`);
}`
          },
          {
            name: "getDiagram",
            signature: "getDiagram(): WasmPersistenceDiagram",
            description: "Get the computed persistence diagram",
            returns: "WasmPersistenceDiagram"
          },
          {
            name: "bettiAt",
            signature: "bettiAt(time: number): Uint32Array",
            description: "Get Betti numbers at a specific filtration value",
            parameters: [{ name: "time", type: "number", description: "Filtration parameter" }],
            returns: "Betti numbers"
          }
        ]
      },
      {
        name: "WasmMorseComplex",
        description: "Morse complex for analyzing critical points of functions. Connects Morse theory to homology.",
        methods: [
          {
            name: "constructor",
            signature: "new WasmMorseComplex()",
            description: "Create a Morse complex",
            returns: "WasmMorseComplex"
          },
          {
            name: "addCriticalPoint",
            signature: "addCriticalPoint(point: WasmCriticalPoint): void",
            description: "Add a critical point to the complex",
            parameters: [{ name: "point", type: "WasmCriticalPoint", description: "Critical point" }]
          },
          {
            name: "getCriticalPoints",
            signature: "getCriticalPoints(): Array<WasmCriticalPoint>",
            description: "Get all critical points",
            returns: "Array of critical points"
          },
          {
            name: "getMinima",
            signature: "getMinima(): Array<WasmCriticalPoint>",
            description: "Get all minima (index 0)",
            returns: "Array of minima"
          },
          {
            name: "getSaddles",
            signature: "getSaddles(): Array<WasmCriticalPoint>",
            description: "Get all saddles (index 1 or n-1)",
            returns: "Array of saddle points"
          },
          {
            name: "getMaxima",
            signature: "getMaxima(): Array<WasmCriticalPoint>",
            description: "Get all maxima (index n)",
            returns: "Array of maxima"
          }
        ]
      },
      {
        name: "TopologyFunctions",
        description: "Standalone functions for computational topology",
        methods: [
          {
            name: "ripsFromDistances",
            signature: "ripsFromDistances(distances: Float64Array, numPoints: number, maxDistance: number): WasmFiltration",
            description: "Build a Vietoris-Rips filtration from a distance matrix",
            isStatic: true,
            parameters: [
              { name: "distances", type: "Float64Array", description: "Flattened n×n distance matrix" },
              { name: "numPoints", type: "number", description: "Number of points" },
              { name: "maxDistance", type: "number", description: "Maximum filtration distance" }
            ],
            returns: "WasmFiltration containing the Rips complex",
            example: `// 3 points with distances
const distances = new Float64Array([0,1,1.5, 1,0,1, 1.5,1,0]);
const filtration = ripsFromDistances(distances, 3, 2.0);`
          },
          {
            name: "findCriticalPoints2D",
            signature: "findCriticalPoints2D(values: Float64Array, width: number, height: number): Array<WasmCriticalPoint>",
            description: "Find critical points of a 2D height function on a grid",
            isStatic: true,
            parameters: [
              { name: "values", type: "Float64Array", description: "Height values on grid" },
              { name: "width", type: "number", description: "Grid width" },
              { name: "height", type: "number", description: "Grid height" }
            ],
            returns: "Array of critical points with type (min/max/saddle) and coordinates",
            example: `// Find critical points of sin(x)sin(y)
const gridSize = 32;
const values = new Float64Array(gridSize * gridSize);
for (let i = 0; i < gridSize; i++) {
  for (let j = 0; j < gridSize; j++) {
    values[i * gridSize + j] = Math.sin(Math.PI * i/gridSize) * Math.sin(Math.PI * j/gridSize);
  }
}
const criticalPoints = findCriticalPoints2D(values, gridSize, gridSize);`
          }
        ]
      }
    ]
  },
  {
    id: "dynamics",
    title: "Dynamical Systems",
    description: "ODE solvers, stability analysis, bifurcations, Lyapunov exponents, and chaos detection",
    icon: "∿",
    classes: [
      {
        name: "RungeKutta4",
        description: "Classic 4th-order Runge-Kutta integrator for ODE systems. Fixed step size with O(h⁴) local error.",
        methods: [
          {
            name: "constructor",
            signature: "new RungeKutta4()",
            description: "Create a new RK4 solver",
            returns: "RungeKutta4",
            example: `const solver = new RungeKutta4();`
          },
          {
            name: "step",
            signature: "step(system: DynamicalSystem, state: Multivector, t: number, dt: number): Multivector",
            description: "Perform a single integration step",
            parameters: [
              { name: "system", type: "DynamicalSystem", description: "The dynamical system" },
              { name: "state", type: "Multivector", description: "Current state" },
              { name: "t", type: "number", description: "Current time" },
              { name: "dt", type: "number", description: "Time step" }
            ],
            returns: "New state after dt"
          },
          {
            name: "solve",
            signature: "solve(system: DynamicalSystem, initial: Multivector, t0: number, t1: number, steps: number): Trajectory",
            description: "Integrate from t0 to t1 with fixed number of steps",
            parameters: [
              { name: "system", type: "DynamicalSystem", description: "The dynamical system" },
              { name: "initial", type: "Multivector", description: "Initial state" },
              { name: "t0", type: "number", description: "Start time" },
              { name: "t1", type: "number", description: "End time" },
              { name: "steps", type: "number", description: "Number of steps" }
            ],
            returns: "Trajectory with time series data",
            example: `const trajectory = solver.solve(lorenz, initial, 0.0, 50.0, 5000);
for (const [t, state] of trajectory.iter()) {
  console.log(\`t=\${t}: x=\${state.get(1)}\`);
}`
          }
        ]
      },
      {
        name: "RKF45",
        description: "Runge-Kutta-Fehlberg adaptive step size integrator. Embedded 4th/5th order pair for error estimation.",
        methods: [
          {
            name: "constructor",
            signature: "new RKF45(tolerance?: number)",
            description: "Create an adaptive RKF45 solver",
            parameters: [{ name: "tolerance", type: "number", description: "Error tolerance (default: 1e-6)" }],
            returns: "RKF45"
          },
          {
            name: "solve",
            signature: "solve(system: DynamicalSystem, initial: Multivector, t0: number, t1: number): Trajectory",
            description: "Integrate with adaptive step size control",
            parameters: [
              { name: "system", type: "DynamicalSystem", description: "The dynamical system" },
              { name: "initial", type: "Multivector", description: "Initial state" },
              { name: "t0", type: "number", description: "Start time" },
              { name: "t1", type: "number", description: "End time" }
            ],
            returns: "Trajectory with variable time steps"
          }
        ]
      },
      {
        name: "DormandPrince",
        description: "Dormand-Prince adaptive integrator (DOPRI5). Industry standard for non-stiff ODEs.",
        methods: [
          {
            name: "constructor",
            signature: "new DormandPrince(config?: DormandPrinceConfig)",
            description: "Create a DOPRI5 solver with optional configuration",
            parameters: [{ name: "config", type: "DormandPrinceConfig", description: "Tolerance and step bounds" }],
            returns: "DormandPrince"
          },
          {
            name: "solve",
            signature: "solve(system: DynamicalSystem, initial: Multivector, t0: number, t1: number): Trajectory",
            description: "Integrate with adaptive step control and dense output",
            returns: "Trajectory with interpolation capability"
          }
        ]
      },
      {
        name: "BackwardEuler",
        description: "Implicit Euler method for stiff systems. A-stable but only first-order accurate.",
        methods: [
          {
            name: "constructor",
            signature: "new BackwardEuler(config?: ImplicitConfig)",
            description: "Create a backward Euler solver",
            parameters: [{ name: "config", type: "ImplicitConfig", description: "Newton iteration settings" }],
            returns: "BackwardEuler"
          },
          {
            name: "solve",
            signature: "solve(system: DynamicalSystem, initial: Multivector, t0: number, t1: number, steps: number): Trajectory",
            description: "Integrate stiff system with implicit method",
            returns: "Trajectory"
          }
        ]
      },
      {
        name: "LorenzSystem",
        description: "The Lorenz attractor - a paradigmatic chaotic system with butterfly-shaped strange attractor.",
        methods: [
          {
            name: "classic",
            signature: "static classic(): LorenzSystem",
            description: "Create Lorenz system with classic parameters (σ=10, ρ=28, β=8/3)",
            isStatic: true,
            returns: "LorenzSystem",
            example: `const lorenz = LorenzSystem.classic();
// Exhibits chaotic behavior with positive Lyapunov exponent`
          },
          {
            name: "constructor",
            signature: "new LorenzSystem(sigma: number, rho: number, beta: number)",
            description: "Create Lorenz system with custom parameters",
            parameters: [
              { name: "sigma", type: "number", description: "Prandtl number (typically 10)" },
              { name: "rho", type: "number", description: "Rayleigh number (chaos for ρ > 24.74)" },
              { name: "beta", type: "number", description: "Geometric factor (typically 8/3)" }
            ],
            returns: "LorenzSystem"
          },
          {
            name: "vectorField",
            signature: "vectorField(state: Multivector): Multivector",
            description: "Compute dx/dt = σ(y-x), dy/dt = x(ρ-z)-y, dz/dt = xy-βz",
            parameters: [{ name: "state", type: "Multivector", description: "Current (x,y,z) state" }],
            returns: "Time derivative"
          }
        ]
      },
      {
        name: "VanDerPolOscillator",
        description: "Self-sustained relaxation oscillator with limit cycle attractor.",
        methods: [
          {
            name: "constructor",
            signature: "new VanDerPolOscillator(mu: number)",
            description: "Create Van der Pol oscillator with damping parameter",
            parameters: [{ name: "mu", type: "number", description: "Nonlinear damping (μ=0: harmonic, μ>0: limit cycle)" }],
            returns: "VanDerPolOscillator",
            example: `const vdp = new VanDerPolOscillator(1.0);
// All trajectories converge to stable limit cycle`
          },
          {
            name: "vectorField",
            signature: "vectorField(state: Multivector): Multivector",
            description: "Compute dx/dt = y, dy/dt = μ(1-x²)y - x",
            returns: "Time derivative"
          }
        ]
      },
      {
        name: "DuffingOscillator",
        description: "Double-well potential oscillator exhibiting bistability and chaos under forcing.",
        methods: [
          {
            name: "constructor",
            signature: "new DuffingOscillator(delta: number, alpha: number, beta: number)",
            description: "Create Duffing oscillator",
            parameters: [
              { name: "delta", type: "number", description: "Damping coefficient" },
              { name: "alpha", type: "number", description: "Linear stiffness" },
              { name: "beta", type: "number", description: "Nonlinear stiffness" }
            ],
            returns: "DuffingOscillator"
          }
        ]
      },
      {
        name: "RosslerSystem",
        description: "Rössler attractor - simpler chaotic system with single-scroll structure.",
        methods: [
          {
            name: "constructor",
            signature: "new RosslerSystem(a: number, b: number, c: number)",
            description: "Create Rössler system",
            parameters: [
              { name: "a", type: "number", description: "Parameter a (typically 0.2)" },
              { name: "b", type: "number", description: "Parameter b (typically 0.2)" },
              { name: "c", type: "number", description: "Parameter c (chaos for c ≈ 5.7)" }
            ],
            returns: "RosslerSystem"
          }
        ]
      },
      {
        name: "HenonMap",
        description: "Discrete-time chaotic map with fractal strange attractor.",
        methods: [
          {
            name: "classic",
            signature: "static classic(): HenonMap",
            description: "Create Hénon map with classic parameters (a=1.4, b=0.3)",
            isStatic: true,
            returns: "HenonMap"
          },
          {
            name: "iterate",
            signature: "iterate(state: Multivector): Multivector",
            description: "Apply one iteration: x_{n+1} = 1 - ax_n² + y_n, y_{n+1} = bx_n",
            returns: "Next state"
          }
        ]
      },
      {
        name: "StabilityAnalyzer",
        description: "Tools for analyzing stability of fixed points via linearization and eigenvalues.",
        methods: [
          {
            name: "findFixedPoint",
            signature: "findFixedPoint(system: DynamicalSystem, guess: Multivector, config?: FixedPointConfig): FixedPointResult",
            description: "Find fixed point using Newton's method with optional damping",
            parameters: [
              { name: "system", type: "DynamicalSystem", description: "The dynamical system" },
              { name: "guess", type: "Multivector", description: "Initial guess" },
              { name: "config", type: "FixedPointConfig", description: "Tolerance and iteration limits" }
            ],
            returns: "FixedPointResult with point and convergence info",
            example: `const result = findFixedPoint(vdp, guess);
if (result.converged) {
  console.log("Fixed point:", result.point);
}`
          },
          {
            name: "computeJacobian",
            signature: "computeJacobian(system: DynamicalSystem, point: Multivector, config?: DiffConfig): Matrix",
            description: "Compute the Jacobian matrix at a point via finite differences",
            parameters: [
              { name: "system", type: "DynamicalSystem", description: "The dynamical system" },
              { name: "point", type: "Multivector", description: "Point to linearize around" }
            ],
            returns: "Jacobian matrix"
          },
          {
            name: "classifyStability",
            signature: "classifyStability(eigenvalues: Array<Complex>): StabilityType",
            description: "Classify fixed point stability from eigenvalues",
            parameters: [{ name: "eigenvalues", type: "Array<Complex>", description: "Jacobian eigenvalues" }],
            returns: "StabilityType (StableNode, UnstableSpiral, Saddle, Center, etc.)"
          }
        ]
      },
      {
        name: "LyapunovSpectrum",
        description: "Compute Lyapunov exponents to detect and quantify chaos.",
        methods: [
          {
            name: "compute",
            signature: "computeLyapunovSpectrum(system: DynamicalSystem, initial: Multivector, config?: LyapunovConfig): LyapunovResult",
            description: "Compute full Lyapunov spectrum using QR method",
            parameters: [
              { name: "system", type: "DynamicalSystem", description: "The dynamical system" },
              { name: "initial", type: "Multivector", description: "Initial condition on attractor" },
              { name: "config", type: "LyapunovConfig", description: "Integration time and reorthogonalization interval" }
            ],
            returns: "LyapunovResult with exponents and convergence info",
            example: `const result = computeLyapunovSpectrum(lorenz, initial);
console.log("Exponents:", result.exponents);
// [0.906, 0.0, -14.57] for classic Lorenz
if (result.exponents[0] > 0) {
  console.log("System is chaotic!");
}`
          },
          {
            name: "sum",
            signature: "sum(): number",
            description: "Sum of all Lyapunov exponents (negative for dissipative systems)",
            returns: "Sum Σλᵢ"
          },
          {
            name: "kaplanYorkeDimension",
            signature: "kaplanYorkeDimension(): number",
            description: "Compute Kaplan-Yorke (Lyapunov) dimension of the attractor",
            returns: "Fractal dimension estimate"
          }
        ]
      },
      {
        name: "BifurcationDiagram",
        description: "Generate bifurcation diagrams showing how system behavior changes with parameters.",
        methods: [
          {
            name: "compute",
            signature: "compute(systemFactory: (param: number) => DynamicalSystem, config: ContinuationConfig): BifurcationDiagram",
            description: "Compute bifurcation diagram by parameter continuation",
            parameters: [
              { name: "systemFactory", type: "Function", description: "Creates system for each parameter value" },
              { name: "config", type: "ContinuationConfig", description: "Parameter range, resolution, transient" }
            ],
            returns: "BifurcationDiagram with branches",
            example: `const config = {
  parameterRange: [2.5, 4.0],
  numPoints: 1000,
  transient: 500,
  samples: 100
};
const diagram = BifurcationDiagram.compute(
  r => new LogisticMap(r),
  config
);`
          },
          {
            name: "branches",
            signature: "branches(): Array<[number, Array<number>]>",
            description: "Get all branches as (parameter, values) pairs",
            returns: "Array of parameter-value pairs"
          },
          {
            name: "detectBifurcations",
            signature: "detectBifurcations(): Array<Bifurcation>",
            description: "Automatically detect bifurcation points",
            returns: "Array of detected bifurcations with type and location"
          }
        ]
      },
      {
        name: "Trajectory",
        description: "Time series data from ODE integration with analysis methods.",
        methods: [
          {
            name: "iter",
            signature: "iter(): Iterator<[number, Multivector]>",
            description: "Iterate over (time, state) pairs",
            returns: "Iterator"
          },
          {
            name: "finalState",
            signature: "finalState(): Multivector | null",
            description: "Get the final state of the trajectory",
            returns: "Final state or null if empty"
          },
          {
            name: "times",
            signature: "times(): Float64Array",
            description: "Get all time values",
            returns: "Array of times"
          },
          {
            name: "states",
            signature: "states(): Array<Multivector>",
            description: "Get all state vectors",
            returns: "Array of states"
          }
        ]
      },
      {
        name: "DynamicsFunctions",
        description: "Standalone utility functions for dynamical systems analysis",
        methods: [
          {
            name: "poincareSection",
            signature: "poincareSection(trajectory: Trajectory, sectionFn: (state: Multivector) => number): Array<Multivector>",
            description: "Compute Poincaré section crossings",
            isStatic: true,
            parameters: [
              { name: "trajectory", type: "Trajectory", description: "Input trajectory" },
              { name: "sectionFn", type: "Function", description: "Section surface g(x)=0" }
            ],
            returns: "States at section crossings"
          },
          {
            name: "computeBasin",
            signature: "computeBasin(system: DynamicalSystem, attractors: Array<Multivector>, gridBounds: Bounds, resolution: number): BasinResult",
            description: "Compute basin of attraction on a grid",
            isStatic: true,
            parameters: [
              { name: "system", type: "DynamicalSystem", description: "The dynamical system" },
              { name: "attractors", type: "Array<Multivector>", description: "Known attractors" },
              { name: "gridBounds", type: "Bounds", description: "Grid boundaries" },
              { name: "resolution", type: "number", description: "Grid resolution" }
            ],
            returns: "Basin indices for each grid point"
          },
          {
            name: "birkhoffAverage",
            signature: "birkhoffAverage(trajectory: Trajectory, observable: (state: Multivector) => number): number",
            description: "Compute Birkhoff (time) average of an observable",
            isStatic: true,
            parameters: [
              { name: "trajectory", type: "Trajectory", description: "Input trajectory" },
              { name: "observable", type: "Function", description: "Observable function f(x)" }
            ],
            returns: "Time-averaged value"
          }
        ]
      }
    ]
  }
];

export function APIReference() {
  const [activeSection, setActiveSection] = useState<string | null>("geometric");
  const [searchQuery, setSearchQuery] = useState("");

  const currentSection = apiSections.find(section => section.id === activeSection);

  return (
    <Container size="xl" py="xl">
      <Stack gap="lg">
        <div>
          <Title order={1}>API Reference</Title>
          <Text size="lg" c="dimmed">
            Complete documentation for 77+ classes and 300+ methods
          </Text>
        </div>

        <SimpleGrid cols={{ base: 2, sm: 3, md: 4, lg: 6 }} spacing="sm">
          {apiSections.map((section) => (
            <Card
              key={section.id}
              withBorder
              p="sm"
              onClick={() => setActiveSection(section.id)}
              style={{
                cursor: 'pointer',
                borderColor: activeSection === section.id ? 'var(--mantine-color-cyan-6)' : undefined,
                backgroundColor: activeSection === section.id ? 'var(--mantine-color-dark-6)' : undefined,
              }}
            >
              <Group gap="xs">
                <Text size="lg" ff="monospace">{section.icon}</Text>
                <div>
                  <Text size="sm" fw={500} lineClamp={1}>{section.title}</Text>
                  <Text size="xs" c="dimmed">{section.classes.length} classes</Text>
                </div>
              </Group>
            </Card>
          ))}
        </SimpleGrid>

        {currentSection && (
          <Card withBorder>
            <Card.Section inheritPadding py="sm" bg="dark.6">
              <Group justify="space-between">
                <div>
                  <Group gap="xs">
                    <Text size="xl" ff="monospace">{currentSection.icon}</Text>
                    <Title order={2}>{currentSection.title}</Title>
                  </Group>
                  <Text size="sm" c="dimmed" mt={4}>{currentSection.description}</Text>
                </div>
                <Badge size="lg" variant="light">{currentSection.classes.length} classes</Badge>
              </Group>
            </Card.Section>
            <Card.Section inheritPadding py="md">
              <Accordion variant="separated">
                {currentSection.classes.map((cls) => (
                  <Accordion.Item key={cls.name} value={cls.name}>
                    <Accordion.Control>
                      <Group justify="space-between" wrap="nowrap">
                        <div>
                          <Text fw={600} ff="monospace" size="md">{cls.name}</Text>
                          <Text size="xs" c="dimmed" lineClamp={1}>{cls.description}</Text>
                        </div>
                        <Badge size="sm" variant="outline">{cls.methods.length} methods</Badge>
                      </Group>
                    </Accordion.Control>
                    <Accordion.Panel>
                      <Text size="sm" c="dimmed" mb="md">{cls.description}</Text>
                      <Stack gap="md">
                        {cls.methods.map((method, idx) => (
                          <Box key={idx} p="sm" bg="dark.7" style={{ borderRadius: 'var(--mantine-radius-sm)' }}>
                            <Group gap="xs" mb="xs" wrap="nowrap">
                              {method.isStatic && <Badge size="xs" color="grape">static</Badge>}
                              <Text fw={600} ff="monospace" size="sm">{method.name}</Text>
                            </Group>
                            <CodeHighlight
                              code={method.signature}
                              language="typescript"
                              withCopyButton={false}
                              mb="xs"
                            />
                            <Text size="sm" c="dimmed" mb="xs">{method.description}</Text>
                            {method.parameters && method.parameters.length > 0 && (
                              <Box mb="xs">
                                <Text size="xs" fw={600} mb={4}>Parameters:</Text>
                                <Stack gap={4}>
                                  {method.parameters.map((p, i) => (
                                    <Group key={i} gap="xs">
                                      <Badge size="xs" variant="light" ff="monospace">{p.name}: {p.type}</Badge>
                                      <Text size="xs" c="dimmed">{p.description}</Text>
                                    </Group>
                                  ))}
                                </Stack>
                              </Box>
                            )}
                            {method.returns && (
                              <Text size="xs"><strong>Returns:</strong> {method.returns}</Text>
                            )}
                            {method.example && (
                              <Box mt="xs">
                                <Text size="xs" fw={600} mb={4}>Example:</Text>
                                <CodeHighlight
                                  code={method.example}
                                  language="javascript"
                                  withCopyButton
                                />
                              </Box>
                            )}
                          </Box>
                        ))}
                      </Stack>
                    </Accordion.Panel>
                  </Accordion.Item>
                ))}
              </Accordion>

              {currentSection.functions && currentSection.functions.length > 0 && (
                <Box mt="lg">
                  <Title order={4} mb="md">Top-Level Functions</Title>
                  <Stack gap="md">
                    {currentSection.functions.map((func, idx) => (
                      <Box key={idx} p="sm" bg="dark.7" style={{ borderRadius: 'var(--mantine-radius-sm)' }}>
                        <Group gap="xs" mb="xs">
                          <Badge size="xs" color="teal">function</Badge>
                          <Text fw={600} ff="monospace" size="sm">{func.name}</Text>
                        </Group>
                        <CodeHighlight
                          code={func.signature}
                          language="typescript"
                          withCopyButton={false}
                          mb="xs"
                        />
                        <Text size="sm" c="dimmed">{func.description}</Text>
                      </Box>
                    ))}
                  </Stack>
                </Box>
              )}
            </Card.Section>
          </Card>
        )}

        <Card withBorder>
          <Card.Section inheritPadding py="xs" bg="dark.6">
            <Title order={3} size="h4">Quick Reference</Title>
          </Card.Section>
          <Card.Section inheritPadding py="md">
            <SimpleGrid cols={{ base: 1, sm: 2, md: 3 }} spacing="md">
              <div>
                <Title order={4} size="sm" mb="xs">Import Pattern</Title>
                <CodeHighlight
                  code={`import init, {
  WasmMultivector,
  WasmDualNumber,
  WasmTropicalNumber,
  // ...
} from '@justinelliottcobb/amari-wasm';

await init();`}
                  language="javascript"
                  withCopyButton
                />
              </div>
              <div>
                <Title order={4} size="sm" mb="xs">Memory Management</Title>
                <Text size="sm" c="dimmed" component="ul" style={{ paddingLeft: '1rem', margin: 0 }}>
                  <li>WASM objects need manual cleanup</li>
                  <li>Call <code>.free()</code> when done</li>
                  <li>Use Float64Array for coefficients</li>
                  <li>Batch operations for performance</li>
                </Text>
              </div>
              <div>
                <Title order={4} size="sm" mb="xs">Common Operations</Title>
                <Text size="sm" c="dimmed" component="ul" style={{ paddingLeft: '1rem', margin: 0 }}>
                  <li>Geometric product for rotations</li>
                  <li>Dual numbers for derivatives</li>
                  <li>Tropical for optimization</li>
                  <li>TDC for LLM embeddings</li>
                </Text>
              </div>
            </SimpleGrid>
          </Card.Section>
        </Card>
      </Stack>
    </Container>
  );
}
