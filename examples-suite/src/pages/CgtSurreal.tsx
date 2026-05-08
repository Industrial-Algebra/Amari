import { Title, Text, Card, Container, Stack, List } from "@mantine/core";
import { ExampleCard } from "../components/ExampleCard";
import { useAmariWasm } from "../hooks/useAmariWasm";

type WasmGameId = unknown;

type WasmCgtArenaInstance = {
  zero(): WasmGameId;
  one(): WasmGameId;
  star(): WasmGameId;
  cut(left: WasmGameId, right: WasmGameId): WasmGameId;
  nimHeap(size: number): WasmGameId;
  formatGame(game: WasmGameId): string;
  compare(left: WasmGameId, right: WasmGameId): string;
  outcome(game: WasmGameId): string;
  isNumeric(game: WasmGameId): boolean;
  grundy(game: WasmGameId): number;
  inspect(game: WasmGameId): {
    birthday(): number;
    canonicalForm(): string;
    outcome(): string;
    isImpartial(): boolean;
    isNumeric(): boolean;
    reachableNodeCount(): number;
  };
};

type WasmDyadicInstance = {
  add(other: WasmDyadicInstance): WasmDyadicInstance;
  format(): string;
  numeratorString(): string;
  exponent(): number;
};

type WasmShortSurrealInstance = {
  add(other: WasmShortSurrealInstance): WasmShortSurrealInstance;
  checkedDiv(other: WasmShortSurrealInstance): WasmShortSurrealInstance;
  format(): string;
  birthday(): number;
  sign(): string;
  toGameIn(arena: WasmCgtArenaInstance): WasmGameId;
};

type AmariWasmModule = Record<string, unknown> & {
  WasmCgtArena?: new () => WasmCgtArenaInstance;
  WasmDyadic?: (new (numerator: number, exponent: number) => WasmDyadicInstance) & {
    fromInteger?: (value: number) => WasmDyadicInstance;
  };
  WasmShortSurreal?: {
    fromInteger?: (value: number) => WasmShortSurrealInstance;
    fromDyadic?: (value: WasmDyadicInstance) => WasmShortSurrealInstance;
    fromGame?: (arena: WasmCgtArenaInstance, game: WasmGameId) => WasmShortSurrealInstance;
  };
};

function missingWasmExports(wasm: AmariWasmModule, names: string[]) {
  return names.filter((name) => wasm[name] === undefined || wasm[name] === null);
}

export function CgtSurreal() {
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
      title: "Short Game Inspection via WASM",
      description: "Build short normal-play games in WasmCgtArena and inspect outcomes, numericity, and canonical form.",
      category: "WASM 0.22.0",
      code: `import init, { WasmCgtArena } from '@justinelliottcobb/amari-wasm';

await init();

const arena = new WasmCgtArena();
const zero = arena.zero();
const one = arena.one();
const half = arena.cut(zero, one);
const star = arena.star();
const heap3 = arena.nimHeap(3);

console.log(arena.formatGame(half));      // {0 | 1}
console.log(arena.compare(zero, one));    // less
console.log(arena.outcome(star));         // next-player-wins
console.log(arena.isNumeric(half));       // true
console.log(arena.grundy(heap3));         // 3`,
      onRun: wasmExample((wasm) => {
        const missing = missingWasmExports(wasm, ["WasmCgtArena"]);
        const Arena = wasm.WasmCgtArena;
        if (missing.length > 0 || !Arena) {
          return "This example requires amari-wasm v0.22.0 WasmCgtArena bindings.";
        }

        const arena = new Arena();
        const zero = arena.zero();
        const one = arena.one();
        const half = arena.cut(zero, one);
        const star = arena.star();
        const heap3 = arena.nimHeap(3);
        const inspection = arena.inspect(heap3);

        return [
          `half = ${arena.formatGame(half)}`,
          `0 compared with 1 = ${arena.compare(zero, one)}`,
          `star outcome = ${arena.outcome(star)}`,
          `half numeric = ${arena.isNumeric(half)}`,
          `nim heap 3 Grundy = ${arena.grundy(heap3)}`,
          `heap inspection = birthday ${inspection.birthday()}, canonical ${inspection.canonicalForm()}`
        ].join('\n');
      })
    },
    {
      title: "Exact Short Surreal Arithmetic via WASM",
      description: "Use WasmDyadic and WasmShortSurreal for exact dyadic short-surreal arithmetic and game conversion.",
      category: "WASM 0.22.0",
      code: `import init, { WasmCgtArena, WasmDyadic, WasmShortSurreal } from '@justinelliottcobb/amari-wasm';

await init();

const half = new WasmDyadic(1, 1);
const quarter = new WasmDyadic(1, 2);
console.log(half.add(quarter).format()); // 3/4

const one = WasmShortSurreal.fromInteger(1);
const threeHalves = one.add(WasmShortSurreal.fromDyadic(half));
console.log(threeHalves.format());       // 3/2
console.log(threeHalves.checkedDiv(WasmShortSurreal.fromInteger(3)).format()); // 1/2

const arena = new WasmCgtArena();
const rebuilt = threeHalves.toGameIn(arena);
console.log(arena.isNumeric(rebuilt));   // true`,
      onRun: wasmExample((wasm) => {
        const missing = missingWasmExports(wasm, ["WasmCgtArena", "WasmDyadic", "WasmShortSurreal"]);
        const Arena = wasm.WasmCgtArena;
        const Dyadic = wasm.WasmDyadic;
        const ShortSurreal = wasm.WasmShortSurreal;
        if (missing.length > 0 || !Arena || !Dyadic || !ShortSurreal?.fromInteger || !ShortSurreal.fromDyadic) {
          return "This example requires amari-wasm v0.22.0 CGT/surreal bindings.";
        }

        const half = new Dyadic(1, 1);
        const quarter = new Dyadic(1, 2);
        const threeQuarters = half.add(quarter);
        const one = ShortSurreal.fromInteger(1);
        const threeHalves = one.add(ShortSurreal.fromDyadic(half));
        const divided = threeHalves.checkedDiv(ShortSurreal.fromInteger(3));
        const arena = new Arena();
        const rebuilt = threeHalves.toGameIn(arena);

        return [
          `1/2 + 1/4 = ${threeQuarters.format()} (numerator ${threeQuarters.numeratorString()}, exponent ${threeQuarters.exponent()})`,
          `1 + 1/2 = ${threeHalves.format()} (birthday ${threeHalves.birthday()}, sign ${threeHalves.sign()})`,
          `(3/2) / 3 = ${divided.format()}`,
          `rebuilt game = ${arena.formatGame(rebuilt)}, numeric = ${arena.isNumeric(rebuilt)}`
        ].join('\n');
      })
    },
    {
      title: "Numeric Game to Short Surreal Conversion",
      description: "Convert the numeric short game {0 | 1} into the exact short surreal 1/2.",
      category: "CGT ↔ Surreal Bridge",
      code: `const arena = new WasmCgtArena();
const zero = arena.zero();
const one = arena.one();
const halfGame = arena.cut(zero, one);

const half = WasmShortSurreal.fromGame(arena, halfGame);
console.log(half.format());    // 1/2
console.log(half.birthday());  // 2`,
      onRun: wasmExample((wasm) => {
        const Arena = wasm.WasmCgtArena;
        const ShortSurreal = wasm.WasmShortSurreal;
        if (!Arena || !ShortSurreal?.fromGame) {
          return "This example requires amari-wasm v0.22.0 WasmShortSurreal.fromGame bindings.";
        }

        const arena = new Arena();
        const zero = arena.zero();
        const one = arena.one();
        const halfGame = arena.cut(zero, one);
        const half = ShortSurreal.fromGame(arena, halfGame);

        return [
          `{0 | 1} formats as ${arena.formatGame(halfGame)}`,
          `converted short surreal = ${half.format()}`,
          `birthday = ${half.birthday()}`
        ].join('\n');
      })
    }
  ];

  return (
    <Container size="lg" py="xl">
      <Stack gap="lg">
        <div>
          <Title order={1}>CGT & Short Surreals</Title>
          <Text size="lg" c="dimmed">
            Version 0.22.0 exposes short normal-play games and exact dyadic short surreal numbers through amari-wasm.
          </Text>
        </div>

        <Card withBorder>
          <Card.Section withBorder inheritPadding py="sm">
            <Title order={3}>Release Scope</Title>
          </Card.Section>
          <Card.Section inheritPadding py="md">
            <Text mb="sm">
              The browser surface is intentionally focused: it demonstrates the stable short-game and short-surreal layer without expanding into loopy games, misère play, symbolic surreals, or surcomplex arithmetic.
            </Text>
            <List size="sm" spacing="xs">
              <List.Item><strong>WasmCgtArena</strong>: named small games, cuts, comparison, outcomes, nimbers, and inspection.</List.Item>
              <List.Item><strong>WasmDyadic</strong>: exact dyadic arithmetic used by short surreals.</List.Item>
              <List.Item><strong>WasmShortSurreal</strong>: exact short-surreal arithmetic plus conversion to and from numeric CGT games.</List.Item>
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
