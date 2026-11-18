# Probabilistic Contracts (amari-flynn)

*Added in v0.19.0*

SMT-LIB2 proof obligation generation, Monte Carlo verification, probabilistic value tracking, and rare event classification.

## Overview

The Flynn WASM bindings enable browser-based formal verification workflows:
- Generate `.smt2` proof obligations for download and external solving (Z3, CVC5)
- Run Monte Carlo statistical verification directly in the browser
- Track probabilistic values and classify rare events

## Quick Start

```typescript
import init, {
  WasmSmtProofObligation,
  flynnHoeffdingObligation,
  flynnPreconditionObligation,
  WasmMonteCarloVerifier,
  WasmProb,
  WasmRareEvent
} from '@justinelliottcobb/amari-wasm';

async function flynnDemo() {
  await init();

  // Generate a Hoeffding bound proof obligation
  const obligation = flynnHoeffdingObligation("sample_mean", 1000, 0.1, 0.05);
  const smtOutput = obligation.toSmtlib2();
  console.log(smtOutput); // Complete SMT-LIB2 for Z3/CVC5

  // Verify statistically via Monte Carlo
  const result = obligation.verifyWithMonteCarlo(10000);
  console.log(`Result: ${result}`); // "Verified", "Violated", or "Inconclusive"

  // Build custom obligations
  const custom = new WasmSmtProofObligation(
    "input_valid",
    "Verify input is positive with P >= 0.95",
    "precondition",
    0.95,  // probability bound
    0.0    // unused for precondition
  );
  custom.addVariable("x", "Real");
  custom.addAssertion("(> x 0.0)", "x is positive");
  console.log(custom.toSmtlib2());

  // Monte Carlo verification
  const verifier = new WasmMonteCarloVerifier(10000);
  const estimate = verifier.estimateProbability(10000, 0.7);
  console.log(`P = ${estimate[0].toFixed(3)} [${estimate[1].toFixed(3)}, ${estimate[2].toFixed(3)}]`);

  const check = verifier.verifyProbabilityAtLeast(0.8, 0.5);
  console.log(`P(success) >= 0.5: ${check}`); // "Verified"

  // Probabilistic values
  const coinFlip = WasmProb.withProbability(0.5, 1.0);
  console.log(`P = ${coinFlip.probability()}, value = ${coinFlip.value()}`);

  const doubled = coinFlip.map(2.0);
  console.log(`Doubled: P = ${doubled.probability()}, value = ${doubled.value()}`);

  // Rare event tracking
  const critHit = new WasmRareEvent(0.05, "critical_hit");
  console.log(`${critHit.description()}: ${critHit.classify(0.1)}`); // "Rare"
  console.log(`Is rare at 1%? ${critHit.isRare(0.01)}`); // false

  // Clean up
  obligation.free(); custom.free();
}

flynnDemo();
```

## API Reference

### WasmSmtProofObligation

Build and export SMT-LIB2 proof obligations.

- `new(name, description, kind, param1, param2)`: Create obligation
  - `kind`: `"precondition"` (param1=probability), `"postcondition"` (param1=probability), `"expected_value"` (param1=expected, param2=epsilon), `"concentration"` (param1=samples, param2=epsilon)
- `addVariable(name, sort)`: Declare variable (`"Real"`, `"Int"`, `"Bool"`)
- `addAssertion(expr, comment)`: Add SMT-LIB2 s-expression assertion
- `toSmtlib2()`: Generate complete SMT-LIB2 string
- `verifyWithMonteCarlo(samples)`: Statistical verification bridge, returns `"Verified"` / `"Violated"` / `"Inconclusive"`

### Convenience Constructors

- `flynnHoeffdingObligation(name, n, epsilon, delta)`: Hoeffding concentration bound
- `flynnPreconditionObligation(name, condition, probability)`: Precondition bound
- `flynnPostconditionObligation(name, condition, probability)`: Postcondition bound
- `flynnExpectedValueObligation(name, expected, epsilon, samples)`: Expected value bound

### WasmMonteCarloVerifier

Bernoulli-parameterized statistical verification.

- `new(samples)`: Create verifier
- `estimateProbability(trials, successProb)`: Returns [estimate, lower, upper] (95% CI)
- `verifyProbabilityAtLeast(successProb, bound)`: Check P(success) >= bound, returns `"Verified"` / `"Violated"` / `"Inconclusive"`

### WasmProb

Probabilistic value wrapper.

- `new(value)`: Create certain value (P=1.0)
- `withProbability(p, value)`: Create with specified probability
- `probability()`: Get probability
- `value()`: Get value
- `map(factor)`: Scale value, preserve probability
- `andThen(otherProb, otherValue)`: Combine (multiply probabilities and values)
- `sample()`: Returns value with probability p, or NaN

### WasmRareEvent

Rare event tracking and classification.

- `new(probability, description)`: Create rare event (P must be in (0,1))
- `probability()`: Get probability
- `description()`: Get description
- `isRare(threshold)`: Check if P < threshold
- `classify(threshold)`: Returns `"Impossible"`, `"Rare"`, or `"Probable"`

## Use Cases

- **Formal Verification**: Generate SMT-LIB2 proof obligations for browser-based verification workflows
- **Statistical Testing**: Monte Carlo verification of probability bounds in WASM
- **Game Balance**: Verify that loot drops, critical hits, and other random mechanics stay within design bounds
- **Reliability Engineering**: Bound failure probabilities in web-based monitoring tools
