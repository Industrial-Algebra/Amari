import { Title, Text, Card, Container, Stack, List } from "@mantine/core";
import { ExampleCard } from "../components/ExampleCard";
import { useAmariWasm } from "../hooks/useAmariWasm";

type WasmRationalSurrealInstance = {
  add(other: WasmRationalSurrealInstance): WasmRationalSurrealInstance;
  checkedDiv(other: WasmRationalSurrealInstance): WasmRationalSurrealInstance;
  format(): string;
};

type WasmRationalSurrealStatic = {
  fromRatio(numerator: number, denominator: number): WasmRationalSurrealInstance;
};

type WasmRationalSurcomplexInstance = {
  add(other: WasmRationalSurcomplexInstance): WasmRationalSurcomplexInstance;
  checkedDiv(other: WasmRationalSurcomplexInstance): WasmRationalSurcomplexInstance;
  format(): string;
};

type WasmRationalSurcomplexStatic = {
  fromParts(
    real: WasmRationalSurrealInstance,
    imag: WasmRationalSurrealInstance
  ): WasmRationalSurcomplexInstance;
};

type WasmExperimentalEpsilonRationalInstance = {
  add(other: WasmExperimentalEpsilonRationalInstance): WasmExperimentalEpsilonRationalInstance;
  mul(other: WasmExperimentalEpsilonRationalInstance): WasmExperimentalEpsilonRationalInstance;
  compare(other: WasmExperimentalEpsilonRationalInstance): string;
  format(): string;
  checkedReciprocal(): WasmExperimentalEpsilonRationalInstance;
};

type WasmExperimentalEpsilonRationalStatic = {
  epsilon(): WasmExperimentalEpsilonRationalInstance;
  fromScalar(
    value: WasmRationalSurrealInstance
  ): WasmExperimentalEpsilonRationalInstance;
};

type AmariWasmModule = Record<string, unknown> & {
  WasmRationalSurreal?: WasmRationalSurrealStatic;
  WasmRationalSurcomplex?: WasmRationalSurcomplexStatic;
  WasmExperimentalEpsilonRational?: WasmExperimentalEpsilonRationalStatic;
};

function missingWasmExports(wasm: AmariWasmModule, names: string[]) {
  return names.filter((name) => wasm[name] === undefined || wasm[name] === null);
}

export function Surcomplex() {
  const { ready, error, amari } = useAmariWasm();

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
      title: "Exact Rational Surreal Arithmetic",
      description:
        "Use WasmRationalSurreal to compute 1/3 + 1/6 = 1/2 with exact rational arithmetic.",
      category: "WASM 0.23.0",
      code: `import init, { WasmRationalSurreal } from '@justinelliottcobb/amari-wasm';

await init();

const a = WasmRationalSurreal.fromRatio(1, 3);
const b = WasmRationalSurreal.fromRatio(1, 6);
const sum = a.add(b);

console.log(sum.format()); // 1/2`,
      onRun: wasmExample((wasm) => {
        const missing = missingWasmExports(wasm, ["WasmRationalSurreal"]);
        const RationalSurreal = wasm.WasmRationalSurreal;
        if (missing.length > 0 || !RationalSurreal?.fromRatio) {
          return "This example requires amari-wasm v0.23.0 WasmRationalSurreal bindings.";
        }

        const a = RationalSurreal.fromRatio(1, 3);
        const b = RationalSurreal.fromRatio(1, 6);
        const sum = a.add(b);

        return [
          `1/3 format = ${a.format()}`,
          `1/6 format = ${b.format()}`,
          `1/3 + 1/6 = ${sum.format()}`
        ].join('\n');
      })
    },
    {
      title: "Exact Surcomplex Division",
      description:
        "Use WasmRationalSurcomplex for exact division: 1 / (1 + 1/2 i) = 4/5 - 2/5i.",
      category: "WASM 0.23.0",
      code: `import init, { WasmRationalSurreal, WasmRationalSurcomplex } from '@justinelliottcobb/amari-wasm';

await init();

const zero = WasmRationalSurreal.fromRatio(0, 1);
const one = WasmRationalSurreal.fromRatio(1, 1);
const half = WasmRationalSurreal.fromRatio(1, 2);

const numerator = WasmRationalSurcomplex.fromParts(one, zero);
const denominator = WasmRationalSurcomplex.fromParts(one, half);
const result = numerator.checkedDiv(denominator);

console.log(result.format()); // 4/5 - 2/5i`,
      onRun: wasmExample((wasm) => {
        const missing = missingWasmExports(wasm, ["WasmRationalSurreal", "WasmRationalSurcomplex"]);
        const RationalSurreal = wasm.WasmRationalSurreal;
        const RationalSurcomplex = wasm.WasmRationalSurcomplex;
        if (missing.length > 0 || !RationalSurreal?.fromRatio || !RationalSurcomplex?.fromParts) {
          return "This example requires amari-wasm v0.23.0 surcomplex bindings.";
        }

        const zero = RationalSurreal.fromRatio(0, 1);
        const one = RationalSurreal.fromRatio(1, 1);
        const half = RationalSurreal.fromRatio(1, 2);

        const numerator = RationalSurcomplex.fromParts(one, zero);
        const denominator = RationalSurcomplex.fromParts(one, half);
        const result = numerator.checkedDiv(denominator);

        return [
          `numerator = ${numerator.format()}`,
          `denominator = ${denominator.format()}`,
          `1 / (1 + 1/2i) = ${result.format()}`
        ].join('\n');
      })
    },
    {
      title: "Experimental Epsilon Infinitesimal Ordering",
      description:
        "Use WasmExperimentalEpsilonRational to verify 0 < ε, ε² < ε, and 1/ε is infinite-scale.",
      category: "WASM 0.23.0",
      code: `import init, { WasmRationalSurreal, WasmExperimentalEpsilonRational } from '@justinelliottcobb/amari-wasm';

await init();

const zero = WasmExperimentalEpsilonRational.fromScalar(
  WasmRationalSurreal.fromRatio(0, 1)
);
const epsilon = WasmExperimentalEpsilonRational.epsilon();
const epsilonSquared = epsilon.mul(epsilon);
const oneOverEpsilon = epsilon.checkedReciprocal();

console.log(epsilon.compare(zero));           // greater
console.log(epsilon.compare(epsilonSquared)); // greater
console.log(epsilon.format());
console.log(oneOverEpsilon.format());`,
      onRun: wasmExample((wasm) => {
        const missing = missingWasmExports(wasm, [
          "WasmRationalSurreal",
          "WasmExperimentalEpsilonRational"
        ]);
        const RationalSurreal = wasm.WasmRationalSurreal;
        const EpsilonRational = wasm.WasmExperimentalEpsilonRational;
        if (missing.length > 0 || !RationalSurreal?.fromRatio || !EpsilonRational?.epsilon || !EpsilonRational?.fromScalar) {
          return "This example requires amari-wasm v0.23.0 experimental epsilon bindings.";
        }

        const zero = EpsilonRational.fromScalar(RationalSurreal.fromRatio(0, 1));
        const epsilon = EpsilonRational.epsilon();
        const epsilonSquared = epsilon.mul(epsilon);
        const oneOverEpsilon = epsilon.checkedReciprocal();

        return [
          `ε = ${epsilon.format()}`,
          `ε² = ${epsilonSquared.format()}`,
          `ε compared with 0 = ${epsilon.compare(zero)}`,
          `ε compared with ε² = ${epsilon.compare(epsilonSquared)}`,
          `1/ε = ${oneOverEpsilon.format()}`
        ].join('\n');
      })
    }
  ];

  return (
    <Container size="lg" py="xl">
      <Stack gap="lg">
        <div>
          <Title order={1}>Rational Surreal &amp; Surcomplex</Title>
          <Text size="lg" c="dimmed">
            Version 0.23.0 exposes exact rational surreal arithmetic, rational surcomplex division, and experimental epsilon infinitesimal ordering through amari-wasm.
          </Text>
        </div>

        <Card withBorder>
          <Card.Section withBorder inheritPadding py="sm">
            <Title order={3}>Release Scope</Title>
          </Card.Section>
          <Card.Section inheritPadding py="md">
            <Text mb="sm">
              The 0.23.0 browser surface adds three new WASM classes that work with exact rational arithmetic
              at arbitrary precision, extending beyond the short dyadic layer:
            </Text>
            <List size="sm" spacing="xs">
              <List.Item>
                <strong>Rational Surreals</strong>: exact rational surreal numbers via WasmRationalSurreal, supporting
                addition, checked division, and canonical formatting.
              </List.Item>
              <List.Item>
                <strong>Surcomplex Division</strong>: exact rational surcomplex numbers via WasmRationalSurcomplex,
                with pair-based construction and checked division yielding results like 4/5 - 2/5i.
              </List.Item>
              <List.Item>
                <strong>Experimental Epsilon</strong>: infinitesimal ordering via
                WasmExperimentalEpsilonRational, confirming 0 &lt; ε, ε² &lt; ε, and that 1/ε is on an infinite scale.
              </List.Item>
            </List>
          </Card.Section>
        </Card>

        {examples.map((example) => (
          <ExampleCard key={example.title} {...example} />
        ))}
      </Stack>
    </Container>
  );
}
