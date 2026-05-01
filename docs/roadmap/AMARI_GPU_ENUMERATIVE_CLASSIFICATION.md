# amari-gpu Enumerative Method Classification

Status: 0.20.0 representative audit pass complete. This table classifies the current public `EnumerativeGpuOps` methods by implementation path and API honesty. The broad `amari_gpu::enumerative` module remains public for compatibility; high-use types are also re-exported at crate root.

Legend:

- **GPU formula**: real WGSL compute path, but formula may be compact/bounded.
- **GPU lookup**: real WGSL path returning a documented finite lookup/table result.
- **GPU exact-with-layout-limits**: real WGSL path expected to match the finite combinatorial baseline within stated fixed-size layout limits.
- **GPU heuristic/scaffold**: real WGSL path with intentionally simplified/prototype mathematical semantics.
- **Needs parity expansion**: representative tests exist, but exhaustive CPU-baseline parity is still pending.

## Core enumerative methods

| Method | Input / output | Current path | Classification | Layout / semantic limits | Test coverage | 0.20.0 action |
|---|---:|---|---|---|---|---|
| `batch_intersection_numbers` | `GpuIntersectionData -> f32` | WGSL compact degree/codimension formula | GPU formula; needs parity expansion | Returns `0` if codimension sum exceeds ambient dimension; otherwise `degree1 * degree2 * multiplicity_factor + genus_correction` | Unit + public API representative test | Keep public; document as compact GPU path |
| `batch_schubert_numbers` | `GpuSchubertClass -> f32` | WGSL simplified factorial/Pieri-inspired estimate | GPU heuristic/scaffold | Up to 8 partition parts; not full Schubert calculus parity | Unit smoke test | Keep for compatibility; add deeper CPU parity before claiming exactness |
| `batch_gromov_witten_invariants` | `GpuGromovWittenData -> f32` | WGSL simplified virtual-dimension formula | GPU heuristic/scaffold | Compact genus/degree/target-dimension formula; not full GW invariant engine | Unit smoke test | Keep public; document approximation/prototype semantics |
| `batch_lr_coefficients` | `GpuLittlewoodRichardsonData -> u32` | WGSL size/containment/skew-size rule | GPU heuristic/scaffold | Up to 8 parts per partition; not full LR tableau enumeration | Unit smoke test | Keep public; add CPU LR parity test before exactness claims |
| `batch_namespace_configurations` | `GpuNamespaceData -> u32` | WGSL codimension/dimension classifier | GPU formula/scaffold | Up to 4 capabilities × 4 partition entries; encodes positive-dimensional cases as `1_000_000 + dimension` | Unit smoke test | Keep public; document sentinel encoding |
| `batch_tropical_intersections` | `GpuTropicalSchubertData -> f32` | WGSL total-weight/dimension classifier | GPU formula/scaffold | Up to 8 weights; returns `-1.0` as positive-dimensional sentinel | Unit smoke test | Keep public; document sentinel encoding |
| `batch_multi_intersect` | `GpuMultiIntersectData -> u32` | WGSL total-codimension classifier with one classical special case | GPU formula/scaffold | Up to 4 classes × 8 parts; `1_000_000 + dimension` positive-dimensional sentinel | Unit + public representative test indirectly via existing unit suite | Keep public; add CPU parity cases |
| `batch_wdvv_curve_counts` | `GpuWDVVData -> u32` | WGSL lookup table | GPU lookup | Kontsevich numbers for `P²`, degrees `1..=6`; higher degrees return `0` | Unit + public API representative test | Good v1; document finite lookup |
| `batch_localization_euler_classes` | `GpuLocalizationData -> f32` | WGSL product formula | GPU exact-with-layout-limits | Up to 8 subset entries/weights; computes product over `i in I`, `j not in I` of `(t_j - t_i)` | Unit + public API representative test | Good v1; add CPU parity across edge cases |
| `batch_matroid_ranks` | `GpuMatroidRankData -> u32` | WGSL bitmask max-intersection | GPU exact-with-layout-limits | Ground elements <32; up to 32 bases; rank computed as `max_B |A ∩ B|` | Unit + public API representative test | Good v1; add CPU parity/property tests |
| `batch_csm_euler_characteristics` | `GpuCSMData -> i32` | WGSL constant Schubert-cell contribution | GPU formula/scaffold | Up to 8 partition parts; returns `1` per cell contribution, not whole-variety CSM aggregation | Unit + public API representative test | Keep public; document as cell contribution |
| `batch_operad_multiplicities` | `GpuOperadData -> u32` | WGSL codimension matching | GPU exact for narrow rule | Single interface codimension match within Grassmannian dimension gives `1`; otherwise `0` | Unit + public API representative test | Good narrow v1; document rule |
| `batch_stability_phases` | `GpuStabilityData -> f32` | WGSL normalized `atan2` phase | GPU formula | Phase = `atan2(trust * dim, -codim) / π`, normalized to `[0,1]` | Unit + public API representative test | Good v1; add tolerance/hardware validation |
| `batch_stability_checks` | `GpuStabilityData -> u32` | WGSL phase interval check | GPU formula | Stable iff normalized phase is strictly in `(0,1)` | Unit + public API representative test | Good v1; add boundary tests |

## GF(2)-gated enumerative methods

These are available only with `--features enumerative,gf2`. They are classified here but will be hardened alongside the main `gf2` pass.

| Method | Input / output | Current path | Classification | Layout / semantic limits | Test coverage | Next action |
|---|---:|---|---|---|---|---|
| `batch_finite_field_points` | `GpuFiniteFieldPointData -> u32` | WGSL Gaussian binomial | GPU exact-with-overflow-limits | `u32` arithmetic; no prime-power validation for `q`; overflow possible for larger parameters | Unit smoke test | Add validation/overflow docs and CPU baseline tests |
| `batch_weight_distributions` | `GpuWeightDistributionData -> flattened u32 histogram` | WGSL exhaustive `2^k` codeword enumeration | GPU exact-with-layout-limits | Up to 16 generator rows, 32 columns; output always 33 bins per code | Unit smoke test | Add public API test and CPU histogram parity |
| `batch_kl_coefficients` | `GpuKLPolynomialData -> flattened i32 coefficients` | WGSL subset/Möbius characteristic-polynomial coefficients | GPU formula, not full KL polynomial | Up to 32 encoded bases; enumerates at most `min(2^n, 65536)` subsets; output 16 coeffs/matroid | Unit smoke test | Rename/document as characteristic-polynomial coefficients or add true KL parity later |
| `batch_representability_checks` | `GpuRepresentabilityData -> u32` | WGSL exhaustive `[I|A]` candidate search | GPU exact/inconclusive for narrow normalized search | `n <= 16`, `rank <= 8`, `n-rank <= 8`; returns `2` for inconclusive | Unit smoke test | Add explicit docs/tests for `2 = inconclusive` |

## Conversion constructors

| Constructor | Current behavior | Classification | Limits / caveats |
|---|---|---|---|
| `From<&ChowClass> for GpuIntersectionData` | Copies degree/dimension and leaves second operand/ambient fields zero for caller fill-in | Conversion scaffold | Caller must set pair/ambient fields before meaningful intersection computation |
| `From<&SchubertClass> for GpuSchubertClass` | Copies up to 8 partition parts and Grassmannian dimensions | Lossy fixed-layout conversion | Parts after 8 are truncated |
| `GpuLittlewoodRichardsonData::from_partitions` | Copies λ, μ, ν parts | Lossy fixed-layout conversion | Up to 8 parts each |
| `From<&Namespace> for GpuNamespaceData` | Encodes up to 4 capabilities × 4 partition entries and total codimension | Lossy fixed-layout conversion | Additional capabilities/parts are truncated |
| `GpuTropicalSchubertData::from_schubert` | Uses Schubert partition as tropical weights | Fixed-layout conversion | Up to 8 weights |
| `GpuMultiIntersectData::from_classes` | Encodes up to 4 classes × 8 parts | Lossy fixed-layout conversion | Additional classes/parts are truncated |
| `GpuWDVVData::from_degree` | Stores degree as `u32` | Narrow conversion | Large `u64` degrees truncate to `u32` |
| `GpuLocalizationData::from_fixed_point` | Encodes fixed-point subset and torus weights | Lossy fixed-layout conversion | Up to 8 subset entries/weights |
| `GpuMatroidRankData::from_matroid_subset` | Bitmask-encodes subset and up to 32 bases | Fixed-layout conversion | Elements >=32 and bases after 32 are ignored |
| `GpuCSMData::from_partition` | Copies partition and Grassmannian params | Lossy fixed-layout conversion | Up to 8 parts |
| `GpuOperadData::from_composition` | Encodes selected interface codimensions; missing indices map to 0 | Narrow conversion | Missing interfaces silently become codimension 0 |
| `GpuStabilityData::from_class_and_trust` | Encodes codimension, dimension, trust level | Formula input conversion | Uses `f32`; class dimension from `k*(n-k)` |
| GF(2) enumerative constructors | Bitmask/fixed-layout encodings | Fixed-layout conversion | Same limits as GF(2)-gated methods |

## Release posture

For 0.20.0, enumerative should be described as:

> Broad GPU-backed enumerative kernels with representative public API tests and explicit fixed-layout/compact-formula semantics. High-use exact-with-layout-limits paths are covered; deeper CPU parity remains a post-v1 hardening task.

Do not market the whole module as a complete symbolic enumerative geometry engine until the heuristic/scaffold rows above have CPU-baseline parity or are narrowed/renamed.
