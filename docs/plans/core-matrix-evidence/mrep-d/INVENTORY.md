# M-REP-D measured inventory (raw evidence)

Develop dedd1f6 + #314. Commands and full site list per contract §1.
Reproduced 2026-10-17:
- unsafe site census: 5 genuine files (simd.rs, aligned_alloc.rs,
  metric.rs, connection.rs x2), ~11 unsafe blocks total
- false positives dispositioned: discovery metadata fields (8),
  parser test fixture string (2), gpu subset_len field name (5)
- dead infrastructure: aligned_alloc module (0 users), AlignedBuffer (0 users)
- construction invariant verified: from_coefficients asserts len==BASIS_COUNT
  (lib.rs:238-244), boxed slice — S1 in-bounds argument holds
