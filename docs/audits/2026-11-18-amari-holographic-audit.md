# Phase A · amari-holographic correctness audit

**Baseline:** `audit/holographic` at `10f60c19b11cfbda5d42fb37c1483be4ece8c320` (the audited develop tree). **Disposition:** findings for operator review, not an approved implementation contract. No library changes. The requested audit date is used in the filename. `src/` below means `amari-holographic/src/` unless qualified.

## Finding that determines the order of repairs

The recommended binding backend is **not Clifford multiplication**. Its three generators square to +1 and its coefficient order differs deliberately from core, but after an explicit layout conversion 16 of the 64 basis products have the wrong sign. Associativity fails on 142 of the 512 basis triples. This propagates to both serial and parallel `ProductCliffordAlgebra`. Fixing isolated retrieval examples, or making two backends agree with one another, would not establish correctness. Start with a sign oracle independent of both.

**Scope and chronology.** 20 Rust source files, 8,355 lines; 155 inline tests and `tests/superposition.rs`. Programme #352 identified `tracedecay` as an external consumer of the 0.23 line; no claim that it exercises any particular defect. Read extraction `23482bf` (2025-12-22: bindings, memory and laws) before `93d064a` / `fea3ecc` (2025-12-29: optical fields, Lee and docs), dependency update `852dde4`, additive storage `c91f7d2` (#189), discovery/docs `cbc7a48`, then W02 `49d944a` / lint `8276bda`. Inspect the current arithmetic, not just its originating diff. Neither FHRR nor MAP purports to be a different implementation of Clifford multiplication.

**Baseline execution.** `rustc 1.100.0 (0ef1bc386 2026-11-09)`, x86_64 Linux. Use `env -u CARGO_ENCODED_RUSTFLAGS -u CARGO_BUILD_TARGET -u RUSTC_WRAPPER -u RUSTC_WORKSPACE_WRAPPER cargo +1.100.0`. `test -p amari-holographic` and `--all-features` each returned 0: 155 unit + 8 integration passed, 21 ignored documentation examples (zero executed). This is the pre-repair suite, not evidence that the identities below hold. `check -p amari-holographic --lib --no-default-features` returned 101; see B1. Library tests, concrete oracle observations and statistical guidance are different levels of evidence.

## Extraction: Clifford kernel, wrapper and product algebra

**Replay:** [executable](holographic-repro/src/bin/clifford.rs) and [observations](holographic-repro/clifford.out). From the root run the Cargo invocation above with `run --manifest-path docs/audits/holographic-repro/Cargo.toml --bin clifford`; set `RAYON_NUM_THREADS=1` to replay the reduction example. The panic hook on stderr for the caught `K=0` test is expected. No random input is needed for the basis tests.

**Oracle.** `amari-core/src/lib.rs:217–232,251–267,423–449` uses masks, positive/negative/null metric and the Cayley product. Local slots `[1,e1,e2,e3,e12,e13,e23,I]` map to masks `[0,1,2,4,3,5,6,7]`. In Euclidean signature an independent oracle counts inversions when concatenating sorted basis words, removes repeated letters, and emits sign `(-1)^swaps` on mask `a XOR b`. It agrees exactly with core on all 64 products. `Cl3::reverse` and Clifford conjugation agree on **all 8** blades with grade reversal and reversal followed by parity respectively. `product_clifford.rs` is a direct product (not a tensor product); no cross-factor geometric terms should appear.

| ID | Grade | Source and counterexample (actual → expected) |
|---|---|---|
| C1 | P1 | `algebra/cl3.rs:158–237`, `product_clifford.rs:153–161,253–262`: `(e1 e3)e2=+I`, `e1(e3 e2)=−I`; both must be `−I`. E.g. `e13 e2` should introduce one swap. The same 16 wrong entries occur in a serial and a Rayon component. Public `bind`, `Mul`, versor composition, norms of resulting products and unbinding consume this kernel. The module's printed table is not an independent oracle. |
| C2 | P1 | `cl3.rs:278–296`, `product_clifford.rs:190–197`: blindly using `reverse(x)/<x reverse(x)>₀` admits the zero divisor `1+e1`; it returns a candidate whose product is `1+e1`, not 1. `(1+e1)(1−e1)=0`. Even the invertible `2+e1` returns a candidate producing `1+0.8e1`; core returns `(2−e1)/3`. These examples involve only the *correct* scalar/e1 subtable, so C1's repair alone cannot fix C2. A coefficient unit vector in R⁸ is not a unit versor. |
| C3 | P1 | `cl3.rs:607–628`: fixed output-grade filters are not contraction or wedge. `1 ∧ e1=0` instead of `e1`; `e1 ⋅ I=0` instead of `e23`. Core selects grade `r+s` or `abs(r−s)` **per homogeneous input pair** (`amari-core/src/lib.rs:455–499`). With current product, 21/64 wedge and 16/64 inner cases disagree with core. |
| C4 | P1 | `cl3.rs:399–404,603–605`, `clifford.rs:325–335`, trait `mod.rs:435–436`: three meanings of dual. For `e12`, optimized and wrapped both give `−e3`, core Hodge gives `+e3`. Local intends `x I⁻¹`, wrapper documents that but computes `x I`; neither implements core's `reverse(x) I` on all grades. `Cl(0,0,1)` null generator silently maps to zero in wrapper where core returns an error. Keep right Clifford dual, metric Hodge, and formal complement conceptually distinct (`amari-core/src/lib.rs:1515–1549`). |
| C5 | P1 | `cl3.rs:440–452`, inherited by product `permute_components`: claimed cyclic extension sends `e12→e13`; but `e1→e2`, `e2→e3`, so their wedge sends `e12→e23`. A reversible shuffle of coefficient slots is not the induced signed exterior map. Test preservation of products *and* the action on generators. |
| C6 | P1 | `clifford.rs:212–224` divides a signed metric pairing by Euclidean coefficient norms. In `Cl(0,1,0)`, `sim(e1,e1)=−1`; in `Cl(0,0,1)` it is `0`. Neither is the advertised cosine `+1`. Legitimate negative or null metric square is not a retrieval similarity. |
| C7 | P2 | `clifford.rs:103–147`: `random_versor(1)` advertises an invertible product of metric unit vectors; in `Cl(0,0,1)` its inverse fails for **every** draw. Draws are coefficient-normalized without regard to the metric. A fallible sampler or a restricted signature/key domain is needed; documentation of probabilistic orthogonality is not a fix. |
| C8 | P2 | `cl3.rs:278–282,511–535`, `product_clifford.rs:180–187,390–413`, `clifford.rs:212–234`: finite `1e200` has representable magnitude; optimized norm reports infinity, normalization succeeds with scalar 0, cosine of itself is NaN. `1e−200` norm vanishes. Full wrapper computes a stable norm but self-cosine still yields NaN or 0 respectively. Separate normalized coefficient dot, metric scalar product, absolute thresholds, nonfinite/zero handling and reciprocal overflow. |
| C9 | P1 | `product_clifford.rs:266–279`: `par_bundle_all([1,1], beta=0)` gives **0.5**, sequential gives **1** (one Rayon thread). Zero is not an identity for averaging, and weighted binary bundling is not associative; scheduling cannot repair this. Do not discard a worker error with `unwrap_or(a)`. |
| C10 | P2 | `product_clifford.rs:223–239,415–422`: `K=0` identity constructs, `permute(0)` panics on remainder by zero; invalid `shuffle_components([0,2])` silently changes `[2,3]` into `[2,0]`. Const array length does not enforce `K>0` or bijectivity. Specify whether shuffling is checked permutation or indexed gather. |

**B1 — P2, feature contract:** crate root `lib.rs:269`, `Cargo.toml:26–31`, e.g. `optical/algebra.rs:152`, `optical/rotor_field.rs:102`, `optical/tropical.rs:177`. The declared `no_std` configuration fails even on host in a library-only build (43 diagnostics in this run, including `cannot find macro vec` and unresolved `std`). No claim about untested targets. Decide supported `alloc`/target matrix or remove the unsupported promise.

**P3 examples, not blockers on their own.** `product_clifford.rs:29–38,199–210,390–404` documents an extra `K` factor: with K=2 and both inputs identity the implementation's coefficient cosine is 1, the documented expression is 1/2. `cl3.rs:47–62` shows an ignored `amari_fusion` import in the extracted crate. `algebra/mod.rs:409–416` calls reversion grade involution despite giving the correct reversal formula (on `e1` they have opposite signs). `cl3.rs:278–282` computes a full product and takes only the scalar; `product_clifford.rs:199–210` repeats the pattern per component. Consider direct coefficient reductions after fixing semantics; no measured speedup claimed.

## Extraction: FHRR, MAP and the generic laws

Replay `cargo +1.100.0 test --release --manifest-path docs/audits/holographic-repro/Cargo.toml --test spectral -- --nocapture --test-threads=1` with the environment above. [Cases](holographic-repro/tests/spectral.rs) · [observed output](holographic-repro/spectral.out).

**Positive control:** FHRR takes complex **frequency bins**. There is no missing FFT normalization in its multiplication. Under an unscaled forward DFT and inverse scaled by `1/D`, IDFT of bound spectra matched independent circular convolution within `2e−15` on both real and complex test inputs. For `[1,2,3,4]` the time energy is 30 and spectral energy 120; a unit-modulus key preserves both, a general key need not. A bin rotation is not a time delay. MAP's component multiplication recovers values exactly with bipolar keys.

| ID | Grade | Source and counterexample |
|---|---|---|
| F1 | P1 | `algebra/fhrr.rs:364–377`, `map.rs:332–341`, `mod.rs:201–224`: conjugation and rebinding are inverses only for component-unitary or bipolar keys respectively. Spectra `[2,3i]` and MAP `[2,−3]` each recover `[4,9]` instead of `[1,1]`; both return `Ok` for the zero key. FHRR's distinct component reciprocal works on this example. `normalize` does not imply a unit phasor in every bin. |
| F2 | P2 | `fhrr.rs:232–267,437–443`, `map.rs:201–208,401–407`: just as in C8, `[1e200]` normalizes to zero and NaN is accepted. FHRR distinguishes global L2 normalization from per-bin projection, but the latter preserves zero bins instead of establishing a unitary key. |
| F3 | P2 | `fhrr.rs:285–304`, `map.rs:231–241`: for `D=0`, `permute(1)` panics in both. For FHRR nonempty all-ones spectrum, `permute(1)` leaves IDFT as impulse at index 0, not a delayed impulse; the comments conflate spectral and temporal shifts. |
| F4 | P2 | `map.rs:185–197,343–384`: `bundle([2],[0],1)` gives .942681; `bundle_all` on the same two gives .761594. For `β=0`, bundling `[2]` with itself yields zero, not an unaltered average. Specify hard ties and whether list cleanup agrees with the pair. |
| F5 | P2 | `algebra/mod.rs:154–165,516–535`: binding need not distribute over *nonlinear cleanup*: for bipolar `−1` and a hard vote on `(+1,−1)`, the two sides yield `−1` and `+1`. Dissimilarity is statistical, not an identity. The stated zero-temperature-parameter limit of `logsumexp(0,0,β)` is not the average: 0 at β=0, 693147.18056 at `1e−6` (`ln 2 / β`). Distinguish log-sum-exp from log-mean-exp. |

`algebra/tests.rs:29–63,293–385` skips some error branches and uses cosine where exact scale recovery matters; `tests/superposition.rs:121–243` checks real and imaginary contributions, a better pattern. P3: FHRR exposes a real-only coefficient view, not a complex serialization; its `from_vec` diagnostics can say expected 2, actual 2 when the real vector has length 3 (`fhrr.rs:78–84,449–482`). Binding and bundling of already-spectral bins are O(D), not an FFT per invocation. No general `Deserialize` for MAP/FHRR under `serialize`; attempted decodes fail trait checking, not runtime validation.

## Extraction: memory, reconsidered against the #189 additive contract

[Cases](holographic-repro/src/bin/memory.rs) · [output](holographic-repro/memory.out); run the evidence crate's `--bin memory` in release. The noncommutative *single-key* retrieval uses the correct side for `k·v`: `e1` retrieves `e2` exactly. With mixed-grade `2+e1`, the raw output `e2+.8e12` instead of `e2` is explained by C2, not by cross-talk.

| ID | Grade | Source and counterexample |
|---|---|---|
| M1 | P1 | `memory/holographic_memory.rs:124–141,157–178,260–275`: store, batch and merge use attention bundling instead of additive superposition. Three basis values at β=0 yield weights `(¼,¼,½)` sequentially or `(½,¼,¼)` grouped, not `(1,1,1)` or a uniform rescaling. At hard selection an item can disappear completely while count advances. |
| M2 | P1 | `holographic_memory.rs:189–199,224–242,311–334`: empty memory reports an absent key present with confidence 1. Eight orthogonal Walsh-sign keys (ProductCl3x32, common identity value, estimated capacity 46) give oldest/newest recall cosine `.000082/.965734`, identical confidence `.940894`. This disproves query-specific interpretation, **not** an asymptotic random-key capacity bound. State the key distribution, success criterion and noise model, or label D/ln D and √(D/n) as load heuristics. |
| M3 | P1 | `holographic_memory.rs:130–141,157–178,189–199`: singular key retrieval replaces error with zero and passes `verify_result_validity` with confidence `.756883`. Store/merge have analogous fallbacks before count changes. `memory/error.rs:7–37` contains a failure vocabulary these APIs do not propagate. Make explicit when a failed operation may mutate state. |
| M4 | P2 | `holographic_memory.rs:218–221,269–275,290–302`: all-false and all-true masks return identical nonzero raw recall. Merge one untracked item into an empty tracked memory: count=1, `verify_consistency=false`. Document or implement partial coordinates and the tracking merge policy. |
| M5 | P1 | `memory/resonator.rs:190–211`: factorizing `A·B` needs `bound·B⁻¹` for A; the implementation uses `B⁻¹·bound`. With left book `[e1,−e1]`, right `[e2]`, result has reconstruction cosine **−1** and reports convergence after two iterations. This counterexample does not require mixed-grade keys. |
| M6 | P1 | `resonator.rs:101–152,230–245,270–283`: an orthogonal input converges on iteration 1 at similarity 0 before any update. At `initial_beta=final_beta=∞`, a one-iteration exact-match lookup `[e1,e2]` for `e1` returns index 1. Finite positive softmax followed by argmax is not a weighted update; stagnation is not a residual certificate. |

`memory/mod.rs:6–29` and `lib.rs:138–193` advertise automatic cleanup and nearest-entry convergence; the register specifies exactly which paths differ. `trace()` supplies neither tracking state nor a whole-memory serde round trip; lack of such an API is a scope question, distinct from the malformed optical deserialization below.

## Optical contribution: phase, envelope, sampling and restore

[Cases](holographic-repro/src/bin/optical.rs) · [actual results](holographic-repro/optical.out); use `--release --bin optical`. These are numerical/sampled checks of a 0/1 amplitude mask, **not** hardware measurements. Treat `E=A exp(iφ)` as the input specified in `optical/mod.rs`; if the desired input is instead pulse width, the transfer formula is a description rather than a correction to that encoder.

| ID | Grade | Source and counterexample |
|---|---|---|
| O1 | P1 | `optical/lee_encoder.rs:175–181`, `rotor_field.rs:81–83`: the threshold sets duty `D=A`. The transmitted arc has normalized first-order magnitude `sin(πD)/π`, not proportional to A. At amplitude ¼ and ¾ the magnitude is identical (.225102), and at unit amplitude, phase .37, the mask is all on and the first order vanishes. Ordinary phase-only field construction therefore does not encode the intended phase in that order. |
| O2 | P1 | `lee_encoder.rs:225–241`: for unit incident illumination, first-order *power* for a 0/1 grating is `sin²(πD)/π²`. Half duty, 1,024 pixels, carrier bin 8: measured normalized DFT power `.10134153`, ideal `.10132118`, API `.40528470`. This is not a rounding error or a sign convention. |
| O3 | P2 | `lee_encoder.rs:24–28,125–137`: opposite constant phases ±.37 produce distinct patterns at bin 8 but identical ones at DC and Nyquist. Ramps move the first order from bin 8 to 11, DC or Nyquist as predicted. In the horizontal case even separation from DC/Nyquist needs signal bandwidth below `min(f, ½−f)` cycles/pixel; not sufficient for arbitrary aperture/harmonics/aliasing. State what is an encoding versus a recoverable order. |
| O4 | P1 | `optical/algebra.rs:115–116,264–281`: reverse conjugates phase but leaves amplitude. Key .5, value .8 unbinds to .2, not .8. Reverse is an inverse on unit-amplitude rotors only. |
| O5 | P1 | `optical/tropical.rs:58–129,215–238`: selecting phases by distance to identity and multiplying by rotation does not distribute: phases `a=1,b=2,c=−2` give −1 vs 0. Swapping equal-distance ±1 also changes the winner. The soft selector on 1 and 2 at β=200 gives a NaN amplitude with unit-amplitude inputs. Distinguish ordered min-plus costs from a heuristic on a circle and stabilize finite soft weights. |
| O6 | P2 | `tropical.rs:254–265,289–299`: step ignores its current state; starting exactly at stored phase 2 in `[1,2]` returns 1. A fixed result independent of the input is not a demonstration of a recall basin. |
| O7 | P2 | `algebra.rs:113–114,169–177,235–245`, `rotor_field.rs:188–213`: 1-pixel self-cosine 0 at amplitudes `1e−6` and `1e10`. Repeated self-binding a unit-envelope rotor 28 times gives rotor norm 10.51, self-similarity 110.41. Public mutable component access can also immediately invalidate the unit rotor. Preserve/check both scalar-bivector norm and scale separately. |
| O8 | P2 | `rotor_field.rs:60–62,148–169`, `tropical.rs:59,185–189`, `hologram.rs:70–82,202–206`: invalid column `(2,0)` in a 2×2 field returns `(0,1)`; a 1×1 tropical algebra accepts a 2×2 min result; normalized Hamming on mismatched empty/nonempty operands can panic. On this 64-bit release build `(2^63,2)` makes length 0. Shape, checked area, axis bounds and empty algebra policy must agree at constructors, operations and restore. |
| O9 | P2 | `rotor_field.rs:36–45`, `hologram.rs:35–40,160–171`: derived serde accepts 2×2 field with zero amplitudes and a one-pixel mask with all 8 bits set (fill 8 instead of ≤1). In contrast, the valid `codebook.rs:271–304` checkpoint replay regenerates the same field. Validate lengths, padding and coefficient policy at the wire boundary. |
| O10 | P2 | `codebook.rs:233–244,262–265`: seed derivation feeds native `Hash` bytes into a deterministic hash. Same base+symbol `X`: little-endian `6ad4880886c80eb8`, big-endian byte **model** `8ff883bd769f9f50`; this is not an execution on a big-endian host. Persisted explicit seeds are distinct from the cross-platform claim about deriving new ones. |

`optical/tests.rs:119–133,209–254` changes the default unit envelope and never decodes its purported round trip. The scalar/bivector product and the sign of in-band phase in `lee_encoder.rs` are consistent with the DFT; `hologram.rs` packs valid grids row-major and masks tail bits in ordinary constructors. `codebook.rs:126–141,207–225` invalidates cached entries on updates; a seed is not a dimensioned field. `optical/mod.rs:18–50` correctly describes complex phase representation but conflates conjugate and general inverse. These observations delimit each finding; they do not imply every use is wrong.

## Proposed remediation contracts — decisions reserved to the operator

| Bounded package | Decision to record before implementation | Acceptance evidence |
|---|---|---|
| H1 · Clifford kernel & geometric operations (C1–C5) | Delegate through an explicit slot/mask adapter, or retain a generated/verified fixed kernel? Which distinct public maps are Hodge, Clifford right dual and formal complement? | Independent 64-pair/512-triple checks; all homogeneous grade products and signed automorphism; full coefficient two-sided residual on mixed invertible keys, rejection of zero divisors; CPU/GPU consumer parity plan. |
| H2 · binding domain & numerical policy (C6–C10, F1–F5) | Separate typed keys from arbitrary traces or expose checked inverses/adjoints on general elements? Which empty/finite/β domains and parallel list semantics are supported? | Direct DFT and independent real/complex oracles, tiny/huge/zero cases, unit versus invertible keys, deterministic Rayon parity, signed/degenerate metric examples. No certified key constructible by unchecked mutation/restore. |
| H3 · memory & resonator (M1–M6) | Additive trace, attention memory or separately named models? Is confidence a nominal load score or validated query evidence? Real annealing/weighted updates or explicitly nearest-neighbor search? | Store/batch/merge oracle, noncommuting factor recovery, empty/singular distinctions, masked queries, tracking, no false convergence or silent failure, measured recall vs load with stated assumptions. |
| H4 · optical field and Lee contract (O1–O4, O7–O9) | Is the encoder accepting normalized complex amplitude, duty cycle, or distinct entry points? Which signed order, incident-power units, noninvertible envelope, shape/serde policy? | Direct DFT on actual bits, including endpoints and band edges; checked shape/packed bits, valid and invalid restore; verify JS-side shape/failure handling. |
| H5 · optional optical selectors & portable configurations (O5–O6, O10, B1) | Ordered tropical algebra or named phase heuristics? Platform-stable seed protocol or local-only generation? Support a real no-std target matrix or explicitly narrow the promise? | Law/negative controls, no NaN for valid soft weights, state-sensitive recall if retained, seed/version fixtures, host feature matrix and named supported targets. |

Separate contract review and implementation review; run rather than infer counterexamples, cap the loop at 12 rounds, require zero unresolved P1/P2 in each approved scope. Establish owner and milestone for deferred items. Whether any package joins 0.25.0 is a separate operator release decision. Nothing here authorizes a fix.

## Consumers and verification boundary

`amari-wasm/src/fusion.rs:823–876` constructs product keys from arbitrary coefficient arrays; `amari-wasm/src/optical.rs:30–46` exposes the optics. `amari-gpu/src/holographic.rs:6–12,30–37` has active CPU fallbacks **and** separate GPU kernels for `ProductCl3x32`. `amari-fusion/src/lib.rs:119–131` reexports, `amari-discovery/src/probes/holographic.rs:4–9,105–132` uses MAP and publishes capacity/attributions. These are impact references, not tests of external consumers or hardware. A change to signed multiplication is a behavioral migration even if arrays keep their layout: regenerate stored bindings/traces, check CPU/WGSL against the independent oracle, exercise WASM in Node, and avoid a silent reinterpretation of durable data. Do not widen this audit to a GPU rewrite, promise that downstream clients are unaffected, or choose the 0.25.0 release scope here.

## Operator decisions (2026-11-24, recorded at review)

1. **H1 kernel repair:** verified local CL(3) table — regenerate the kernel
   against the independent inversion-counting oracle (which agrees with
   amari-core on all 64 products); 64-pair + 512-triple oracle checks gate
   the fix in CI. No adapter to the generic core product on the binding hot
   path.
2. **Release scope:** H1 and its consumer migrations (wasm fusion key
   construction, gpu CPU-fallback + WGSL parity, stored-binding migration
   disclosure) join **0.25.0**.
3. **Sequencing:** H1 and H3 contract in parallel (H3 next because M1
   contradicts the merged #189 additive-superposition contract); H2/H4/H5
   queue behind.

Evidence status: all replay bins re-executed independently by the
orchestrator after the audit moment returned — clifford (16/64, 142/512,
serial=Rayon), memory (empty→confidence 1, resonator cosine −1
"converged"), optical (power .10134153 measured vs .40528470 API), spectral
positive controls 4/4. Register accepted as evidence baseline.
