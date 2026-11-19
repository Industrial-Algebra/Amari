import init, {
  WasmSmtProofObligation,
  flynnHoeffdingObligation,
  flynnPreconditionObligation,
  flynnPostconditionObligation,
  flynnExpectedValueObligation,
  WasmMonteCarloVerifier,
  WasmProb,
  WasmRareEvent
} from '@justinelliottcobb/amari-wasm';

/**
 * Probabilistic Contracts (amari-flynn) Examples
 *
 * This example demonstrates:
 * - SMT-LIB2 proof obligation generation for formal verification
 * - Monte Carlo statistical verification in WASM
 * - Probabilistic value tracking and composition
 * - Rare event classification
 */
async function runProbabilisticContractsExample() {
  console.log('📋 Probabilistic Contracts (Flynn) Examples');
  console.log('============================================');

  // Initialize the WASM module
  await init();

  // ========================================================================
  // 1. Hoeffding Bound Proof Obligation
  // ========================================================================

  console.log('\n1. Hoeffding Bound Proof Obligation:');

  // Generate SMT-LIB2 for Hoeffding's inequality:
  // P(|X̄ - μ| >= ε) <= 2·exp(-2nε²) <= δ
  const hoeffding = flynnHoeffdingObligation("sample_mean", 1000, 0.1, 0.05);
  const smtOutput = hoeffding.toSmtlib2();

  // Show first few lines of SMT-LIB2 output
  const smtLines = smtOutput.split('\n');
  console.log('   SMT-LIB2 output (first 5 lines):');
  for (let i = 0; i < Math.min(5, smtLines.length); i++) {
    console.log(`   ${smtLines[i]}`);
  }
  console.log(`   ... (${smtLines.length} total lines)`);

  // Verify statistically
  const hoeffdingResult = hoeffding.verifyWithMonteCarlo(10000);
  console.log(`   Monte Carlo verification: ${hoeffdingResult}`);

  // ========================================================================
  // 2. Precondition and Postcondition Obligations
  // ========================================================================

  console.log('\n2. Pre/Postcondition Obligations:');

  // Precondition: input must be positive with high probability
  const precond = flynnPreconditionObligation(
    "input_positive", "(> x 0.0)", 0.95
  );
  const precondSmt = precond.toSmtlib2();
  console.log(`   Precondition SMT contains "Precondition": ${precondSmt.includes('Precondition')}`);
  console.log(`   Precondition MC result: ${precond.verifyWithMonteCarlo(5000)}`);

  // Postcondition: output satisfies invariant
  const postcond = flynnPostconditionObligation(
    "output_bounded", "(< result 100.0)", 0.99
  );
  console.log(`   Postcondition MC result: ${postcond.verifyWithMonteCarlo(5000)}`);

  // ========================================================================
  // 3. Expected Value Obligation
  // ========================================================================

  console.log('\n3. Expected Value Obligation:');

  // Verify that E[X] = 5.0 ± 0.1 with 10000 samples
  const evObligation = flynnExpectedValueObligation("mean_check", 5.0, 0.1, 10000);
  const evSmt = evObligation.toSmtlib2();
  console.log(`   Expected value SMT contains "mu": ${evSmt.includes('mu')}`);
  console.log(`   MC verification: ${evObligation.verifyWithMonteCarlo(10000)}`);

  // ========================================================================
  // 4. Custom Proof Obligation
  // ========================================================================

  console.log('\n4. Custom Proof Obligation:');

  // Build a custom concentration bound obligation
  const custom = new WasmSmtProofObligation(
    "temperature_stable",
    "Verify temperature stays within safe range with P >= 0.99",
    "concentration",
    500,   // samples
    0.05   // epsilon
  );
  custom.addVariable("temp", "Real");
  custom.addVariable("pressure", "Real");
  custom.addVariable("is_safe", "Bool");
  custom.addAssertion("(>= temp 20.0)", "minimum temperature");
  custom.addAssertion("(<= temp 80.0)", "maximum temperature");
  custom.addAssertion("(=> is_safe (and (>= temp 20.0) (<= temp 80.0)))", "safety definition");

  const customSmt = custom.toSmtlib2();
  console.log(`   Variables declared: temp (Real), pressure (Real), is_safe (Bool)`);
  console.log(`   Assertions: 3`);
  console.log(`   SMT-LIB2 length: ${customSmt.length} characters`);
  console.log(`   Contains check-sat: ${customSmt.includes('(check-sat)')}`);

  // ========================================================================
  // 5. Monte Carlo Verification
  // ========================================================================

  console.log('\n5. Monte Carlo Statistical Verification:');

  const verifier = new WasmMonteCarloVerifier(10000);

  // Estimate probability of a Bernoulli trial
  const est70 = verifier.estimateProbability(10000, 0.7);
  console.log(`   P(success) = 0.7 estimate: ${est70[0].toFixed(3)} [${est70[1].toFixed(3)}, ${est70[2].toFixed(3)}]`);

  const est30 = verifier.estimateProbability(10000, 0.3);
  console.log(`   P(success) = 0.3 estimate: ${est30[0].toFixed(3)} [${est30[1].toFixed(3)}, ${est30[2].toFixed(3)}]`);

  // Verify bounds (hold frame: P(success) >= bound)
  const bound1 = verifier.verifyProbabilityAtLeast(0.8, 0.5);
  console.log(`   P(0.8) >= 0.5: ${bound1}`); // Verified

  const bound2 = verifier.verifyProbabilityAtLeast(0.3, 0.5);
  console.log(`   P(0.3) >= 0.5: ${bound2}`); // Violated

  // ========================================================================
  // 6. Probabilistic Values
  // ========================================================================

  console.log('\n6. Probabilistic Value Tracking:');

  // Certain value (P = 1.0)
  const certain = new WasmProb(100.0);
  console.log(`   Certain value: ${certain.value()} (P = ${certain.probability()})`);

  // Coin flip: value 1.0 with P = 0.5
  const coinFlip = WasmProb.withProbability(0.5, 1.0);
  console.log(`   Coin flip: value = ${coinFlip.value()}, P = ${coinFlip.probability()}`);

  // Map: scale the value, preserve probability
  const doubled = coinFlip.map(2.0);
  console.log(`   Doubled: value = ${doubled.value()}, P = ${doubled.probability()}`);

  const tripled = coinFlip.map(3.0);
  console.log(`   Tripled: value = ${tripled.value()}, P = ${tripled.probability()}`);

  // Compose independent events: P(A and B) = P(A) * P(B)
  const combined = coinFlip.andThen(0.3, 10.0);
  console.log(`   Combined (0.5 * 0.3): P = ${combined.probability().toFixed(2)}, value = ${combined.value().toFixed(1)}`);

  // Sampling (stochastic)
  let samples = 0;
  const numSamples = 1000;
  for (let i = 0; i < numSamples; i++) {
    const s = coinFlip.sample();
    if (!isNaN(s)) samples++;
  }
  console.log(`   Sampling P=0.5 event 1000x: ${samples} hits (expected ~500)`);

  // ========================================================================
  // 7. Rare Event Classification
  // ========================================================================

  console.log('\n7. Rare Event Classification:');

  // Critical hit in a game (5% chance)
  const critHit = new WasmRareEvent(0.05, "critical_hit");
  console.log(`   ${critHit.description()}: P = ${critHit.probability()}`);
  console.log(`   Classify at 10% threshold: ${critHit.classify(0.1)}`);
  console.log(`   Classify at 1% threshold: ${critHit.classify(0.01)}`);
  console.log(`   Is rare at 10%? ${critHit.isRare(0.1)}`);
  console.log(`   Is rare at 1%? ${critHit.isRare(0.01)}`);

  // Server failure (0.1% chance)
  const serverFail = new WasmRareEvent(0.001, "server_failure");
  console.log(`\n   ${serverFail.description()}: P = ${serverFail.probability()}`);
  console.log(`   Classify at 1% threshold: ${serverFail.classify(0.01)}`);
  console.log(`   Classify at 0.01% threshold: ${serverFail.classify(0.0001)}`);

  // Lottery win (extremely rare)
  const lottery = new WasmRareEvent(0.0000001, "lottery_win");
  console.log(`\n   ${lottery.description()}: P = ${lottery.probability()}`);
  console.log(`   Classify at 1% threshold: ${lottery.classify(0.01)}`);
  console.log(`   Is rare at 0.001%? ${lottery.isRare(0.00001)}`);

  // ========================================================================
  // 8. Practical Example: Game Balance Verification
  // ========================================================================

  console.log('\n8. Practical: Game Balance Verification');

  // Verify that the combined probability of multiple rare drops
  // stays within acceptable bounds for player experience

  const legendaryDrop = WasmProb.withProbability(0.02, 100);   // 2% for 100 gold
  const epicDrop = WasmProb.withProbability(0.10, 50);         // 10% for 50 gold
  const rareDrop = WasmProb.withProbability(0.25, 20);         // 25% for 20 gold

  console.log(`   Legendary: ${legendaryDrop.value()} gold @ P=${legendaryDrop.probability()}`);
  console.log(`   Epic: ${epicDrop.value()} gold @ P=${epicDrop.probability()}`);
  console.log(`   Rare: ${rareDrop.value()} gold @ P=${rareDrop.probability()}`);

  // Track the legendary drop as a rare event
  const legendaryEvent = new WasmRareEvent(0.02, "legendary_drop");
  console.log(`   Legendary drop classified as: ${legendaryEvent.classify(0.05)}`);

  // Verify drop rate guarantees with Monte Carlo (hold frame)
  const dropVerifier = new WasmMonteCarloVerifier(50000);
  const atLeastOnePct = dropVerifier.verifyProbabilityAtLeast(0.02, 0.01);
  console.log(`   P(legendary) >= 1%: ${atLeastOnePct}`); // Verified

  const atLeastTenPct = dropVerifier.verifyProbabilityAtLeast(0.02, 0.1);
  console.log(`   P(legendary) >= 10%: ${atLeastTenPct}`); // Violated

  // ========================================================================
  // Clean up WASM memory
  // ========================================================================

  hoeffding.free();
  precond.free();
  postcond.free();
  evObligation.free();
  custom.free();
  certain.free();
  coinFlip.free();
  doubled.free();
  tripled.free();
  combined.free();
  critHit.free();
  serverFail.free();
  lottery.free();
  legendaryDrop.free();
  epicDrop.free();
  rareDrop.free();
  legendaryEvent.free();

  console.log('\n✅ Probabilistic contracts example completed!');
}

// Run the example
runProbabilisticContractsExample().catch(console.error);
