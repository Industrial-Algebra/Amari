//! Exact dyadic square of a bivector (CORE-W03 stage 1).
//!
//! Contract authority: `docs/plans/2026-10-10-amari-core-w03-inverse-exp-contract.md`,
//! section **D02.3 "exp(): exact-domain closed form + series fallback"**
//! (precedence 3, "Exact bivector — exact dyadic square" and its erratum of
//! 2026-10-12). that exact square, the symbolic-scaling rounding pipeline (single destination-precision rounding for scalar-factored exponentials), and the certified exact-polynomial evaluation of nilpotent exponentials.
//!
//! # Why an exact square exists
//!
//! Every finite `f64` is a dyadic rational `c = m * 2^e` with `m` an integer of
//! at most 53 bits. For a bivector `B = sum_i c_i e_i` (over grade-2 basis
//! blades) every output coefficient of `B^2` is a signed sum of products
//! `c_i * c_j`, hence an exact integer times `2^(e_i + e_j)`. `B^2` of a
//! bivector has only a scalar part and a grade-4 part, so the whole square can
//! be accumulated exactly in a fixed-size signed integer. This is the
//! nilpotence (`B^2 = 0`) and simplicity (grade-4 part `= 0`) certificate, and
//! it is also the only magnitude source for the closed form. Compensated
//! summation of rounded terms cannot replace it: rounded normalized terms can
//! sum to exactly zero while the exact square is nonzero (the round-4 crafted
//! case).
//!
//! **No floating-point certificate is used anywhere in routing.** The square
//! is exact or the input was not an exact finite multivector; uncertainty does
//! not exist in this route.
//!
//! # Generalization (round-1 review remediation)
//!
//! [`exact_x2`] computes **every** output coefficient of `x·x` for an
//! arbitrary multivector `x` with the same accumulator, by iterating every
//! ordered pair of basis blades with its general Cayley sign. The bivector
//! entry point [`exact_bivector_square`] delegates to it, so the stage-1
//! bivector API and its oracle tests are unchanged. The generalized square is
//! what certifies `is_exactly_scalar` for the any-scale scalar-square closed
//! form in `exp()` (square-zero vectors, circular/hyperbolic scalar squares).
//! A general output coefficient receives at most `2^dim <= 64` product terms,
//! so the 66-word capacity bound below (proved with headroom for 4096 terms)
//! covers the generalized square as well.
//!
//! # Accumulator and the 66-word capacity
//!
//! The accumulator is a two's-complement signed integer of
//! [`WORDS`]` = 66` `u64` words (4224 bits), with no allocation and no
//! `unsafe`. Its value `V` represents the dyadic quantity `V * 2^`[`BASE_EXP`]
//! with `BASE_EXP = -2148`.
//!
//! Proved bound: the smallest product of two coefficients is
//! `2^-1074 * 2^-1074 = 2^-2148` (two minimum subnormals), and the largest is
//! below `2^2048` (two maximum finites, `(2^53-1)^2 * 2^1942 < 2^2048`),
//! so the aligned integer spans `4196` magnitude bits. Adding one sign bit and
//! headroom for at most `4096` accumulated terms per coefficient (a grade-2
//! bivector in dimension `d` contributes `C(d,2) <= 2016` scalar terms, and a
//! grade-4 output blade receives at most three pairings in two orders) requires
//! at most `4209` signed bits. `66 * 64 = 4224` bits therefore suffice.
//!
//! # Magnitude extraction
//!
//! For a simple (grade-4-free) bivector the exact scalar `V` is normalised as
//! `|V| = M * 2^b`, `M in [1,2)`, and the value is `M * 2^(b + BASE_EXP)`.
//! The top 128 magnitude bits are carried exactly with a sticky flag for the
//! discarded low bits, then rounded to an `f64` with a single round-to-nearest,
//! ties-to-even step (the sticky bit preserves the discarded information, so
//! this is the correctly rounded `f64` of `M`). With `E = b + BASE_EXP`,
//! decompose `E = 2q + r`, `r in {0,1}`, and form the angle
//! `sqrt(M * 2^r) * 2^q`: the `sqrt` is IEEE-correctly-rounded on the
//! `f64`-rounded operand (a mantissa carry into the exponent is handled), and
//! the `2^q` rescale is a single exact power-of-two multiply, which also gives
//! correct round-to-nearest-even subnormal results and maps overflow to the
//! non-finite policy (`+inf`). Single-blade bivectors short-circuit to
//! `|c|` bit-for-bit, without extraction.

use crate::Multivector;
use alloc::vec;
use alloc::vec::Vec;
use core::cmp::Ordering;

/// Number of `u64` words in the fixed signed accumulator (`66 * 64 = 4224` bits).
pub(crate) const WORDS: usize = 66;

/// The aligned integer `V` represents the dyadic value `V * 2^BASE_EXP`.
const BASE_EXP: i32 = -2148;

/// Maximum number of basis blades the fixed accumulator table supports
/// (`2^6 = 64`, matching the contract's `N <= 64` workspace bound). Signatures
/// with more blades report [`ExactSquare::is_unsupported`] and route to the
/// series.
pub(crate) const MAX_BASIS: usize = 64;

/// `2^-52` as an exact `f64`.
const TWO_POW_M52: f64 = f64::from_bits(0x3CB0_0000_0000_0000);

/// Fixed-size two's-complement signed bignum: `WORDS` little-endian `u64`
/// limbs. Bit 4223 is the sign bit. No heap, no `unsafe`.
#[derive(Clone, Copy)]
struct SignedAccumulator {
    words: [u64; WORDS],
}

impl SignedAccumulator {
    /// The all-zero accumulator.
    const fn zero() -> Self {
        Self { words: [0; WORDS] }
    }

    /// Whether every word is exactly zero.
    fn is_zero(&self) -> bool {
        self.words.iter().all(|&w| w == 0)
    }

    /// Sign of the two's-complement value: `-1`, `0`, or `+1`.
    fn sign(&self) -> i32 {
        if self.is_zero() {
            0
        } else if (self.words[WORDS - 1] >> 63) & 1 == 1 {
            -1
        } else {
            1
        }
    }

    /// Absolute value as an unsigned limb array (little-endian).
    fn magnitude(&self) -> [u64; WORDS] {
        let mut out = self.words;
        if (out[WORDS - 1] >> 63) & 1 == 1 {
            let mut carry = true;
            for word in out.iter_mut() {
                *word = !*word;
                if carry {
                    let (value, overflow) = word.overflowing_add(1);
                    *word = value;
                    carry = overflow;
                }
            }
        }
        out
    }

    /// Add the signed term `±(mag << shift)` to the accumulator.
    ///
    /// `mag` is a product of two at-most-53-bit mantissas, hence `< 2^106`.
    /// The term must fit inside the proved capacity (all callers satisfy it).
    fn add_term(&mut self, mag: u128, shift: usize, negative: bool) {
        if mag == 0 {
            return;
        }
        let word = shift / 64;
        let bit = shift % 64;
        debug_assert!(
            word + 2 < WORDS,
            "exact-square term exceeds the 66-word capacity"
        );

        let lo = mag as u64;
        let hi = (mag >> 64) as u64;
        let parts = if bit == 0 {
            [lo, hi, 0]
        } else {
            [
                lo << bit,
                (hi << bit) | (lo >> (64 - bit)),
                hi >> (64 - bit),
            ]
        };

        if negative {
            self.sub_parts(&parts, word);
        } else {
            self.add_parts(&parts, word);
        }
    }

    /// Add an unsigned three-limb value at `word` (carry propagates upward).
    fn add_parts(&mut self, parts: &[u64; 3], word: usize) {
        let mut carry = 0u64;
        for (i, &part) in parts.iter().enumerate() {
            let idx = word + i;
            if idx >= WORDS {
                break;
            }
            let sum = self.words[idx] as u128 + part as u128 + carry as u128;
            self.words[idx] = sum as u64;
            carry = (sum >> 64) as u64;
        }
        let mut idx = word + 3;
        while carry != 0 && idx < WORDS {
            let sum = self.words[idx] as u128 + carry as u128;
            self.words[idx] = sum as u64;
            carry = (sum >> 64) as u64;
            idx += 1;
        }
    }

    /// Subtract an unsigned three-limb value at `word` (borrow propagates upward).
    fn sub_parts(&mut self, parts: &[u64; 3], word: usize) {
        let mut borrow = 0u64;
        for (i, &part) in parts.iter().enumerate() {
            let idx = word + i;
            if idx >= WORDS {
                break;
            }
            let sub = part as u128 + borrow as u128;
            let cur = self.words[idx] as u128;
            if cur >= sub {
                self.words[idx] = (cur - sub) as u64;
                borrow = 0;
            } else {
                self.words[idx] = (cur + (1u128 << 64) - sub) as u64;
                borrow = 1;
            }
        }
        let mut idx = word + 3;
        while borrow != 0 && idx < WORDS {
            let cur = self.words[idx] as u128;
            if cur >= 1 {
                self.words[idx] = (cur - 1) as u64;
                borrow = 0;
            } else {
                self.words[idx] = u64::MAX;
            }
            idx += 1;
        }
    }

    /// Exact top-128 magnitude bits, the MSB index `b`, and the sticky flag for
    /// the discarded lower bits. `None` for the zero accumulator.
    fn extracted(&self) -> Option<(u128, usize, bool)> {
        let mag = self.magnitude();
        let bits = bit_len(&mag);
        if bits == 0 {
            return None;
        }
        let b = bits - 1;
        let (top, sticky) = top_128(&mag, b);
        Some((top, b, sticky))
    }

    /// `sqrt(|V| * 2^BASE_EXP)` within 1 ulp, via the `E = 2q + r` composition.
    fn sqrt_angle(&self) -> f64 {
        let Some((top, b, sticky)) = self.extracted() else {
            return 0.0;
        };
        let (mantissa, carry) = round_mantissa_to_f64(top, sticky);
        let b_eff = b as i64 + i64::from(carry);
        let e = b_eff + BASE_EXP as i64;
        let r = e.rem_euclid(2);
        let q = (e - r) / 2;
        let radicand = if r == 1 { mantissa * 2.0 } else { mantissa };
        scale_pow2(radicand.sqrt(), q)
    }
}

/// Number of significant magnitude bits in a little-endian limb array.
fn bit_len(words: &[u64; WORDS]) -> usize {
    for i in (0..WORDS).rev() {
        if words[i] != 0 {
            return i * 64 + (64 - words[i].leading_zeros() as usize);
        }
    }
    0
}

/// Top 128 magnitude bits, left-aligned (bit 127 = the MSB at index `b`), plus
/// a sticky flag for any discarded lower bit.
fn top_128(words: &[u64; WORDS], b: usize) -> (u128, bool) {
    let mut top = 0u128;
    for k in 0..128 {
        if k > b {
            break;
        }
        let bit = b - k;
        let value = (words[bit / 64] >> (bit % 64)) & 1;
        top |= (value as u128) << (127 - k);
    }

    let mut sticky = false;
    if b >= 127 {
        let below = b - 127;
        for i in 0..below {
            if (words[i / 64] >> (i % 64)) & 1 == 1 {
                sticky = true;
                break;
            }
        }
        // Bits of the top window are already exact; only strictly lower bits
        // contribute to the sticky flag.
    }
    (top, sticky)
}

/// Round the 128-bit mantissa `top` (with external sticky) to the correctly
/// rounded `f64` in `[1,2)`. The second value reports a carry that pushed the
/// significand to `2.0` (the caller then bumps the binary exponent by one).
fn round_mantissa_to_f64(top: u128, sticky: bool) -> (f64, bool) {
    let significand = (top >> 75) as u64; // top 53 bits
    let guard = (top >> 74) & 1 == 1;
    let rest = top & ((1u128 << 74) - 1);
    let sticky = sticky || rest != 0;
    let mut rounded = significand;
    let mut carry = false;
    if guard && (sticky || significand & 1 == 1) {
        rounded += 1;
        if rounded == 1u64 << 53 {
            rounded >>= 1;
            carry = true;
        }
    }
    (rounded as f64 * TWO_POW_M52, carry)
}

/// Multiply `x` by `2^n` with a single correctly-rounded power-of-two factor.
///
/// For this module `x in [1,2)` and `n in [-1074, 1024]`: `n <= 1023` yields a
/// representable power of two (subnormal below `-1022`), and `n > 1023` maps to
/// the overflow (`+inf`) non-finite policy.
fn scale_pow2(x: f64, n: i64) -> f64 {
    if n >= -1022 {
        if n > 1023 {
            return f64::INFINITY;
        }
        x * f64::from_bits(((n + 1023) as u64) << 52)
    } else if n >= -1074 {
        x * f64::from_bits(1u64 << (n + 1074))
    } else if n == -1075 {
        // `x in [1, 2)`: `x * 2^-1075` rounds to the minimum subnormal for
        // `x > 1` and to zero (ties-to-even) for `x == 1`.
        if x > 1.0 {
            f64::from_bits(1)
        } else {
            0.0
        }
    } else {
        0.0
    }
}

/// Decompose a finite `f64` as `±m * 2^e` with `m <= 2^53` (`m` is 0 for zero).
///
/// Returns `(m, e, negative)`, or `None` for a non-finite input. Subnormals use
/// `e = -1074` and the unnormalised mantissa (smaller `m`).
fn decompose(c: f64) -> Option<(u64, i32, bool)> {
    if !c.is_finite() {
        return None;
    }
    let bits = c.to_bits();
    let negative = (bits >> 63) & 1 == 1;
    let biased = ((bits >> 52) & 0x7FF) as i32;
    let fraction = bits & ((1u64 << 52) - 1);
    if biased == 0 {
        Some((fraction, -1074, negative))
    } else {
        Some((fraction | (1u64 << 52), biased - 1075, negative))
    }
}

/// Signature metric sign of the generators shared by two blades: the product of
/// `e_k^2` over `k` in `common`, i.e. `+1` for a positive generator, a sign flip
/// for a negative one, and `0` for a null (degenerate) one.
fn metric_sign(common: usize, p: usize, q: usize, dim: usize) -> i32 {
    let mut sign = 1i32;
    for k in 0..dim {
        if (common >> k) & 1 == 1 {
            if k < p {
                // positive: no factor
            } else if k < p + q {
                sign = -sign;
            } else {
                return 0;
            }
        }
    }
    sign
}

/// General basis-blade product `e_i * e_j` under the algebra's Cayley
/// convention: `(sign, index)` with `index = i ^ j`, or sign `0` when a shared
/// null generator annihilates the product. This generalizes the bivector
/// `eta`/`reorder` pair machinery to blades of every grade.
fn product_sign_and_index(i: usize, j: usize, p: usize, q: usize, dim: usize) -> (i32, usize) {
    let index = i ^ j;
    let sign = reorder_sign(i, j, dim) * metric_sign(i & j, p, q, dim);
    (sign, index)
}

/// Reordering sign of the product of two (necessarily disjoint) basis blades,
/// matching the algebra's Cayley convention (`(-1)^inversions`).
fn reorder_sign(a: usize, b: usize, dim: usize) -> i32 {
    let mut inversions = 0u32;
    for i in 0..dim {
        if (b >> i) & 1 == 1 {
            for j in (i + 1)..dim {
                if (a >> j) & 1 == 1 {
                    inversions += 1;
                }
            }
        }
    }
    if inversions.is_multiple_of(2) {
        1
    } else {
        -1
    }
}

/// Accumulate one signed product term `±c1*c2` into `acc`.
fn add_product(acc: &mut SignedAccumulator, c1: f64, c2: f64, flip: bool) {
    let (Some((m1, e1, n1)), Some((m2, e2, n2))) = (decompose(c1), decompose(c2)) else {
        return;
    };
    if m1 == 0 || m2 == 0 {
        return;
    }
    let mag = (m1 as u128) * (m2 as u128);
    let shift = (e1 + e2 + BASE_EXP.abs()) as usize;
    acc.add_term(mag, shift, n1 ^ n2 ^ flip);
}

/// Exact summary of `x * x` for a general multivector `x`.
///
/// Every output coefficient is accumulated exactly in the fixed signed bignum:
/// `coeffs[i]` is the dyadic value `(x * x)_i * 2^2148`. `is_zero` is the full
/// square-zero certificate (every output coefficient exactly zero).
/// `is_exactly_scalar` certifies that every **non-scalar** output coefficient
/// is exactly zero, so `x^2 = lambda` is a scalar; it is the routing test for
/// the any-scale scalar-square closed form. `grade4_is_zero` is the
/// bivector-simplicity test (for a bivector input the only non-scalar output
/// grade is 4, so it agrees with `is_exactly_scalar`).
#[derive(Clone)]
pub(crate) struct ExactSquare {
    /// Exact output coefficients of `x * x` scaled by `2^2148`.
    coeffs: [SignedAccumulator; MAX_BASIS],
    scalar_is_zero: bool,
    non_scalar_is_zero: bool,
    // Kept as part of the exact-square bivector certificate API; the round-2
    // routing no longer distinguishes simplicity from scalar-square (the
    // scalar-square route subsumes it), but the oracle unit tests still pin it.
    #[allow(dead_code)]
    grade4_is_zero: bool,
    single_blade_angle: Option<f64>,
    non_finite_source: bool,
    unsupported: bool,
}

impl ExactSquare {
    /// Placeholder for a signature whose basis count exceeds [`MAX_BASIS`].
    /// Every certificate is `false` so the caller falls back to the series.
    fn unsupported_signature() -> Self {
        Self {
            coeffs: [SignedAccumulator::zero(); MAX_BASIS],
            scalar_is_zero: false,
            non_scalar_is_zero: false,
            grade4_is_zero: false,
            single_blade_angle: None,
            non_finite_source: false,
            unsupported: true,
        }
    }

    /// Full square-zero certificate: every output coefficient of `x * x` is
    /// exactly zero.
    pub(crate) fn is_zero(&self) -> bool {
        !self.unsupported && self.scalar_is_zero && self.non_scalar_is_zero
    }

    /// Exact scalar-square certificate: every non-scalar output coefficient of
    /// `x * x` is exactly zero (so `x^2` is a scalar `lambda`).
    pub(crate) fn is_exactly_scalar(&self) -> bool {
        !self.unsupported && self.non_scalar_is_zero
    }

    /// Simplicity certificate for a bivector: the exact grade-4 part of `B^2`
    /// is zero.
    ///
    /// Retained for the exact-square oracle tests; the round-2 `exp` routing
    /// uses [`Self::is_exactly_scalar`] for every scalar-square closed form.
    #[allow(dead_code)]
    pub(crate) fn grade4_is_zero(&self) -> bool {
        !self.unsupported && self.grade4_is_zero
    }

    /// `(sign, angle)` of the exact scalar part: `sign` is `-1` (circular), `0`
    /// (zero scalar), or `+1` (hyperbolic); `angle = sqrt(|x^2|)`. For a
    /// single-blade input the original `|c|` is returned bit-for-bit.
    ///
    /// A `+inf`/`NaN` angle signals the non-finite policy (overflow of the
    /// closed form); the caller maps it to `ExpError::NonFiniteResult`.
    pub(crate) fn scalar_sign_and_angle(&self) -> (i32, f64) {
        if self.scalar_is_zero {
            return (0, 0.0);
        }
        let sign = self.coeffs[0].sign();
        let angle = match self.single_blade_angle {
            Some(angle) => angle,
            None => self.coeffs[0].sqrt_angle(),
        };
        (sign, angle)
    }

    /// Symbolic `(sign, gamma, q)` of the exact scalar part, with
    /// `sqrt(|x^2|) = gamma * 2^q` and `gamma in [1, 2*sqrt(2))`.
    ///
    /// Unlike [`Self::scalar_sign_and_angle`], no power-of-two range is
    /// materialized: a scalar square whose root overflows `f64` still yields a
    /// finite `gamma` and an out-of-range `q`. Single-blade inputs keep the
    /// bit-for-bit `|c|` with `q = 0` (CORE-W04 R2-F10).
    pub(crate) fn scalar_sign_and_symbolic_angle(&self) -> (i32, f64, i64) {
        if self.scalar_is_zero {
            return (0, 0.0, 0);
        }
        let sign = self.coeffs[0].sign();
        if let Some(angle) = self.single_blade_angle {
            return (sign, angle, 0);
        }
        match self.coeffs[0].extracted() {
            Some((top, b, sticky)) => {
                let (mantissa, carry) = round_mantissa_to_f64(top, sticky);
                let e = b as i64 + i64::from(carry) + BASE_EXP as i64;
                let r = e.rem_euclid(2);
                let q = (e - r) / 2;
                let radicand = if r == 1 { mantissa * 2.0 } else { mantissa };
                (sign, radicand.sqrt(), q)
            }
            None => (0, 0.0, 0),
        }
    }

    /// Whether a source coefficient was non-finite. The exact route requires
    /// finite coefficients; callers must handle this before routing.
    pub(crate) fn has_non_finite_source(&self) -> bool {
        self.non_finite_source
    }

    /// Whether the signature exceeded the fixed accumulator table
    /// ([`MAX_BASIS`]); callers must route such inputs to the series.
    pub(crate) fn is_unsupported(&self) -> bool {
        self.unsupported
    }
}

/// Compute the exact square `x * x` of an arbitrary multivector.
///
/// Every output coefficient is accumulated exactly in the fixed 66-word signed
/// bignum from the exact dyadic decompositions of the source coefficients: for
/// each ordered pair of basis blades `(i, j)` the product
/// [`product_sign_and_index`] contributes `+-c_i c_j` to blade `i ^ j`. The
/// scalar part is `sum_i eta_i * c_i^2`; when `i == j` the general product sign
/// is exactly `eta_i`. This is the same machinery as the bivector special case
/// (the `index.count_ones() == 2` restriction), generalized to every grade, and
/// is what certifies `is_exactly_scalar` for the any-scale closed form.
///
/// No floating-point certificate is used: the square is exact, or the input was
/// not an exact finite multivector.
pub(crate) fn exact_x2<const P: usize, const Q: usize, const R: usize>(
    x: &Multivector<P, Q, R>,
) -> ExactSquare {
    let dim = P + Q + R;
    let basis_count = 1usize << dim;
    if basis_count > MAX_BASIS {
        return ExactSquare::unsupported_signature();
    }

    let mut coeffs = [SignedAccumulator::zero(); MAX_BASIS];
    let mut nonzero = 0usize;
    let mut single_blade_angle = None;
    let mut non_finite_source = false;

    for i in 0..basis_count {
        let ci = x.get(i);
        if ci == 0.0 {
            continue;
        }
        if !ci.is_finite() {
            non_finite_source = true;
            continue;
        }
        nonzero += 1;
        single_blade_angle = (nonzero == 1).then(|| ci.abs());
        for j in 0..basis_count {
            let cj = x.get(j);
            if cj == 0.0 {
                continue;
            }
            if !cj.is_finite() {
                non_finite_source = true;
                continue;
            }
            let (sign, index) = product_sign_and_index(i, j, P, Q, dim);
            if sign == 0 {
                continue;
            }
            add_product(&mut coeffs[index], ci, cj, sign < 0);
        }
    }

    let mut scalar_is_zero = true;
    let mut non_scalar_is_zero = true;
    let mut grade4_is_zero = true;
    for (index, acc) in coeffs.iter().enumerate().take(basis_count) {
        if !acc.is_zero() {
            if index == 0 {
                scalar_is_zero = false;
            } else {
                non_scalar_is_zero = false;
                if index.count_ones() == 4 {
                    grade4_is_zero = false;
                }
            }
        }
    }

    ExactSquare {
        coeffs,
        scalar_is_zero,
        non_scalar_is_zero,
        grade4_is_zero,
        single_blade_angle,
        non_finite_source,
        unsupported: false,
    }
}

/// Bivector-specialized entry point kept for the stage-1 API and unit tests.
///
/// Delegates to [`exact_x2`]: for a bivector input the non-scalar output part is
/// grade-4 only, so `grade4_is_zero()` and `is_exactly_scalar()` agree. Used by
/// the exact-square oracle tests; the production routing calls [`exact_x2`]
/// directly.
#[allow(dead_code)]
pub(crate) fn exact_bivector_square<const P: usize, const Q: usize, const R: usize>(
    b: &Multivector<P, Q, R>,
) -> ExactSquare {
    exact_x2(b)
}

// ======================================================================
// Round-2 review remediation: exact powers and the any-index nilpotent
// exponential.
//
// Precedence 3c of `Multivector::exp` (see the contract and `lib.rs`)
// requires the exact polynomial `exp(y) = sum_{j<d} y^j / j!` for a
// nilpotent `y` of arbitrary index `d`. The exact square of [`exact_x2`]
// is not enough: its single fixed base exponent cannot represent the higher
// powers `y^3, y^4, ...` across scales. This section keeps every
// coefficient of a power as an exact dyadic `(+/-) m * 2^e` with a fixed
// 66-word magnitude `m` and a per-coefficient binary exponent `e`, and
// multiplies two powers with a schoolbook bignum product ([`mul_exact`]).
// A coefficient that cannot be represented (magnitude capacity or exponent
// overflow) makes `mul_exact` return `None`; the caller then routes to the
// bounded series instead of claiming a certificate it did not obtain.
// ======================================================================

/// Words in the wide intermediate used while multiplying two exact powers.
/// One spare word above the 66-word magnitude product guarantees room for
/// the signed accumulation of the (at most `MAX_BASIS`) terms of one output
/// coefficient.
const WIDE_WORDS: usize = 2 * WORDS + 1;

/// An exact dyadic coefficient `(-1)^negative * magnitude * 2^exponent`.
///
/// `magnitude` is an unsigned 66-word integer. The exponent is a full `i32`
/// so that the higher powers (`e` grows by roughly `j * e_f64`) remain exact.
#[derive(Clone, Copy)]
struct ExactDyadic {
    magnitude: [u64; WORDS],
    negative: bool,
    exponent: i32,
}

impl ExactDyadic {
    const fn zero() -> Self {
        Self {
            magnitude: [0; WORDS],
            negative: false,
            exponent: 0,
        }
    }

    fn is_zero(&self) -> bool {
        self.magnitude.iter().all(|&word| word == 0)
    }

    /// Exact dyadic value of a finite `f64`, or `None` for a non-finite input.
    fn from_f64(c: f64) -> Option<Self> {
        let (mantissa, exponent, negative) = decompose(c)?;
        let mut magnitude = [0u64; WORDS];
        magnitude[0] = mantissa;
        Some(Self {
            magnitude,
            negative,
            exponent,
        })
    }

    /// Correctly rounded `f64` of an exact dyadic `(-1)^negative * magnitude *
    /// 2^exponent`.
    ///
    /// Kept as a unit-test oracle for the extraction machinery (the production
    /// nilpotent route now divides its exact integer accumulator and rounds
    /// once through [`wide_rational_to_f64`]).
    #[cfg(test)]
    #[allow(dead_code)]
    fn to_f64(self) -> f64 {
        if self.is_zero() {
            return 0.0;
        }
        let bits = bit_len(&self.magnitude);
        let b = bits - 1;
        let (top, sticky) = top_128(&self.magnitude, b);
        let (mantissa, carry) = round_mantissa_to_f64(top, sticky);
        let exponent = b as i64 + i64::from(carry) + self.exponent as i64;
        let value = scale_pow2(mantissa, exponent);
        if self.negative {
            -value
        } else {
            value
        }
    }
}

/// Multiply the representable `f64` `x` by `2^exponent` with a single
/// round-to-nearest-even step.
///
/// This is the `ldexp`-style final leg of the round-3 symbolic scalar scale:
/// the mantissa leg (`c · 2^f`) is already a rounded `f64`, so adding the
/// integer binary exponent is exact up to the one subnormal/overflow rounding
/// performed here. For an exactly representable product the result is
/// bit-exact; subnormal results round once, and the overflow policy is `±inf`.
pub(crate) fn scale_by_pow2_rne(x: f64, exponent: i64) -> f64 {
    if x == 0.0 || !x.is_finite() {
        return x;
    }
    let (mantissa, e, negative) = decompose(x).expect("finite input");
    if mantissa == 0 {
        return 0.0;
    }
    let b = 63 - mantissa.leading_zeros() as usize;
    let top = (mantissa as u128) << (127 - b);
    let (rounded, carry) = round_mantissa_to_f64(top, false);
    let total = b as i64 + e as i64 + exponent + i64::from(carry);
    let value = scale_pow2(rounded, total);
    if negative {
        -value
    } else {
        value
    }
}

/// Correctly rounded `f64` of `c * scale_mantissa * 2^scale_exponent`.
///
/// This is the round-4 unification of the symbolic scalar composition. The
/// coefficient and the scale mantissa are decomposed into exact integer
/// mantissas and binary exponents, their exact `53 x 53 -> 106`-bit product is
/// formed, and the single round-to-nearest-even decision is taken at the
/// destination precision with the integer exponent composed exactly. It never
/// forms `c * scale_mantissa` in `f64` first, which is what previously
/// overflowed before the exponent could rescue the result (the `f64::MAX`
/// coefficient case) and double-rounded across the subnormal boundary (the
/// minimum-subnormal coefficient case).
pub(crate) fn apply_symbolic_scale(c: f64, scale_mantissa: f64, scale_exponent: i64) -> f64 {
    if c == 0.0 || scale_mantissa == 0.0 {
        return 0.0;
    }
    let (Some((m_c, e_c, negative_c)), Some((m_s, e_s, negative_s))) =
        (decompose(c), decompose(scale_mantissa))
    else {
        return c;
    };
    if m_c == 0 || m_s == 0 {
        return 0.0;
    }
    let product = (m_c as u128) * (m_s as u128);
    let words = [product as u64, (product >> 64) as u64];
    let exponent = e_c as i64 + e_s as i64 + scale_exponent;
    round_words_scaled_to_f64(negative_c ^ negative_s, &words, exponent, false)
}

/// Correctly rounded `f64` of `c * factor * scale_mantissa * 2^scale_exponent`.
///
/// This is the round-5 three-way extension of [`apply_symbolic_scale`] for the
/// circular and finite-hyperbolic closed-form vector coefficients
/// `(sin a / a)·c` and `(sinh a / a)·c`. The coefficient, the closed-form
/// factor, and the symbolic scalar scale are decomposed into exact integer
/// mantissas and binary exponents; their exact `53 x 53 x 53 -> 159`-bit
/// product is formed and the single round-to-nearest-even decision is taken at
/// the destination precision with the integer exponent composed exactly. It
/// never forms `c * factor` (or `c * scale_mantissa`) in `f64` first, which is
/// what overflowed `f64::MAX · sinhc` before the negative scalar exponent could
/// rescue the finite result.
pub(crate) fn apply_symbolic_scale3(
    c: f64,
    factor: f64,
    scale_mantissa: f64,
    scale_exponent: i64,
) -> f64 {
    if c == 0.0 || factor == 0.0 || scale_mantissa == 0.0 {
        return 0.0;
    }
    let (Some((m_c, e_c, negative_c)), Some((m_f, e_f, negative_f)), Some((m_s, e_s, negative_s))) =
        (decompose(c), decompose(factor), decompose(scale_mantissa))
    else {
        return c;
    };
    if m_c == 0 || m_f == 0 || m_s == 0 {
        return 0.0;
    }
    // `m_c · m_f` is at most 106 bits; multiplying by `m_s` (at most 53 bits)
    // gives at most 159, which fits three limbs.
    let pair = (m_c as u128) * (m_f as u128);
    let mut words = [pair as u64, (pair >> 64) as u64, 0u64];
    let mut carry = 0u128;
    for word in words.iter_mut().take(2) {
        let product = (*word as u128) * (m_s as u128) + carry;
        *word = product as u64;
        carry = product >> 64;
    }
    words[2] = carry as u64;
    let exponent = e_c as i64 + e_f as i64 + e_s as i64 + scale_exponent;
    round_words_scaled_to_f64(
        negative_c ^ negative_f ^ negative_s,
        &words,
        exponent,
        false,
    )
}

/// Correctly rounded `f64` of `c * scale_mantissa * 2^scale_exponent / (2·a)`.
///
/// This is the round-5 finalization of the hyperbolic symbolic fallback's
/// vector coefficient `e^{s+a}/(2a)·c`. The divisor `2a` is never formed as an
/// `f64` (it overflows for `a > f64::MAX / 2`): `a = m_a · 2^{e_a}` is
/// decomposed and the exact rational `1/(2a)` is carried as
/// `1/(2·m_a) · 2^{-e_a}`, so the `c = a` case reproduces the scalar
/// `e^{s+a}/2` bit-for-bit instead of suffering the old
/// `a · (m/(2a))` double rounding. The division remainder is folded into the
/// sticky bit and a single round-to-nearest-even is taken at the destination
/// precision.
pub(crate) fn apply_symbolic_scale_over_2a(
    c: f64,
    angle: f64,
    scale_mantissa: f64,
    scale_exponent: i64,
) -> f64 {
    if c == 0.0 || scale_mantissa == 0.0 {
        return 0.0;
    }
    let (Some((m_c, e_c, negative_c)), Some((m_a, e_a, _))) = (decompose(c), decompose(angle))
    else {
        return c;
    };
    if m_c == 0 || m_a == 0 {
        return 0.0;
    }
    // `m_a <= 2^53`, so `2·m_a` fits a `u64`; the exponent `-e_a` is folded into
    // the composed exponent rather than into an `f64` `2a`.
    let divisor = 2 * m_a;
    let mut magnitude = [0u64; WIDE_WORDS];
    magnitude[0] = m_c;
    wide_rational_scaled_to_f64(
        &magnitude,
        e_c - e_a,
        divisor,
        negative_c,
        scale_mantissa,
        scale_exponent,
    )
}

/// Whether any bit at index `>= n` of a wide magnitude is set.
fn wide_bits_at_or_above(words: &[u64; WIDE_WORDS], n: usize) -> bool {
    if n >= WIDE_WORDS * 64 {
        return false;
    }
    let word = n / 64;
    let bit = n % 64;
    if bit > 0 {
        words[word] >> bit != 0 || words[word + 1..].iter().any(|&w| w != 0)
    } else {
        words[word..].iter().any(|&w| w != 0)
    }
}

/// `src << shift` in a wide magnitude, or `None` if any nonzero bit is
/// shifted out of the fixed width.
fn wide_shl(src: &[u64; WIDE_WORDS], shift: usize) -> Option<[u64; WIDE_WORDS]> {
    if shift >= WIDE_WORDS * 64 {
        return src.iter().all(|&w| w == 0).then_some([0u64; WIDE_WORDS]);
    }
    if wide_bits_at_or_above(src, WIDE_WORDS * 64 - shift) {
        return None;
    }
    let word = shift / 64;
    let bit = shift % 64;
    let mut out = [0u64; WIDE_WORDS];
    if bit == 0 {
        for i in (word..WIDE_WORDS).rev() {
            out[i] = src[i - word];
        }
    } else {
        for i in (word..WIDE_WORDS).rev() {
            let lo = src[i - word] << bit;
            let hi = if i - word == 0 {
                0
            } else {
                src[i - word - 1] >> (64 - bit)
            };
            out[i] = lo | hi;
        }
    }
    Some(out)
}

/// `dst += src` over the full wide width. The spare top word makes overflow
/// impossible for the bounded accumulation in [`mul_exact`].
fn wide_add(dst: &mut [u64; WIDE_WORDS], src: &[u64; WIDE_WORDS]) {
    let mut carry = 0u64;
    for i in 0..WIDE_WORDS {
        let sum = dst[i] as u128 + src[i] as u128 + carry as u128;
        dst[i] = sum as u64;
        carry = (sum >> 64) as u64;
    }
    debug_assert_eq!(carry, 0, "wide accumulator overflow");
}

/// `dst -= src` over the full wide width. Callers guarantee `dst >= src`.
fn wide_sub(dst: &mut [u64; WIDE_WORDS], src: &[u64; WIDE_WORDS]) {
    let mut borrow = 0u64;
    for i in 0..WIDE_WORDS {
        let sub = src[i] as u128 + borrow as u128;
        let cur = dst[i] as u128;
        if cur >= sub {
            dst[i] = (cur - sub) as u64;
            borrow = 0;
        } else {
            dst[i] = (cur + (1u128 << 64) - sub) as u64;
            borrow = 1;
        }
    }
    debug_assert_eq!(borrow, 0, "wide subtraction underflow");
}

fn wide_cmp(a: &[u64; WIDE_WORDS], b: &[u64; WIDE_WORDS]) -> Ordering {
    for i in (0..WIDE_WORDS).rev() {
        if a[i] != b[i] {
            return a[i].cmp(&b[i]);
        }
    }
    Ordering::Equal
}

/// Schoolbook product of two fixed 66-word unsigned magnitudes into a wide
/// (133-word) magnitude: `66 x 66` `u64` partial products accumulated through
/// `u128`.
fn mul_magnitude(a: &[u64; WORDS], b: &[u64; WORDS]) -> [u64; WIDE_WORDS] {
    let mut out = [0u64; WIDE_WORDS];
    for i in 0..WORDS {
        if a[i] == 0 {
            continue;
        }
        let mut carry = 0u128;
        for j in 0..WORDS {
            let product = (a[i] as u128) * (b[j] as u128) + out[i + j] as u128 + carry;
            out[i + j] = product as u64;
            carry = product >> 64;
        }
        let mut idx = i + WORDS;
        while carry != 0 && idx < WIDE_WORDS {
            let sum = out[idx] as u128 + carry;
            out[idx] = sum as u64;
            carry = sum >> 64;
            idx += 1;
        }
    }
    out
}

/// Product of one fixed 66-word magnitude with a small (at most 45-bit)
/// multiplier, into the wide form. The multiplication is exact.
fn mul_magnitude_by_u64(a: &[u64; WORDS], multiplier: u64) -> [u64; WIDE_WORDS] {
    let mut out = [0u64; WIDE_WORDS];
    if multiplier == 0 {
        return out;
    }
    let mut carry = 0u128;
    for i in 0..WORDS {
        let product = (a[i] as u128) * (multiplier as u128) + carry;
        out[i] = product as u64;
        carry = product >> 64;
    }
    let mut idx = WORDS;
    while carry != 0 && idx < WIDE_WORDS {
        let sum = out[idx] as u128 + carry;
        out[idx] = sum as u64;
        carry = sum >> 64;
        idx += 1;
    }
    out
}

/// Number of significant magnitude bits in a little-endian limb slice.
fn slice_bit_len(words: &[u64]) -> usize {
    for i in (0..words.len()).rev() {
        if words[i] != 0 {
            return i * 64 + (64 - words[i].leading_zeros() as usize);
        }
    }
    0
}

/// Whether any bit strictly below bit index `k` (0-based) of a little-endian
/// limb slice is set.
fn slice_any_bit_below(words: &[u64], k: usize) -> bool {
    let full = k / 64;
    let bit = k % 64;
    if full > 0 && words[..full.min(words.len())].iter().any(|&w| w != 0) {
        return true;
    }
    if bit > 0 && full < words.len() {
        words[full] & ((1u64 << bit) - 1) != 0
    } else {
        false
    }
}

/// Whether any bit of `words` other than bit `k` is set.
fn slice_has_other_bits(words: &[u64], k: usize) -> bool {
    let full = k / 64;
    let bit = k % 64;
    if words[..full.min(words.len())].iter().any(|&w| w != 0) {
        return true;
    }
    if full < words.len() && words[full] & !(1u64 << bit) != 0 {
        return true;
    }
    words[(full + 1).min(words.len())..].iter().any(|&w| w != 0)
}

/// Bit `k` of a little-endian limb slice.
fn slice_bit(words: &[u64], k: usize) -> bool {
    let word = k / 64;
    word < words.len() && (words[word] >> (k % 64)) & 1 == 1
}

/// Exact value of `words >> shift` as a `u64`. Callers only use this on the
/// top `keep <= 53` bits of an integer, so the result always fits.
fn slice_value_above(words: &[u64], shift: usize) -> u64 {
    let word = shift / 64;
    let bit = shift % 64;
    if word >= words.len() {
        return 0;
    }
    let mut value = if bit == 0 {
        words[word]
    } else {
        words[word] >> bit
    };
    if bit != 0 && word + 1 < words.len() {
        value |= words[word + 1] << (64 - bit);
    }
    value
}

/// `words << shift` into a fresh little-endian limb vector.
fn slice_shl(words: &[u64], shift: usize) -> Vec<u64> {
    if shift == 0 {
        return words.to_vec();
    }
    let word = shift / 64;
    let bit = shift % 64;
    let mut out = vec![0u64; words.len() + word + 1];
    for (i, &limb) in words.iter().enumerate() {
        out[i + word] |= limb << bit;
        if bit != 0 {
            out[i + word + 1] |= limb >> (64 - bit);
        }
    }
    out
}

/// Divide a little-endian limb slice by a `u64`, returning `(quotient,
/// remainder)`. The divisor is small enough that `(remainder << 64) | limb`
/// always fits a `u128`.
fn slice_div_u64(words: &[u64], divisor: u64) -> (Vec<u64>, u64) {
    let mut quotient = vec![0u64; words.len()];
    let mut remainder = 0u128;
    for i in (0..words.len()).rev() {
        let current = (remainder << 64) | words[i] as u128;
        quotient[i] = (current / divisor as u128) as u64;
        remainder = current % divisor as u128;
    }
    while quotient.len() > 1 && *quotient.last().expect("nonempty") == 0 {
        quotient.pop();
    }
    (quotient, remainder as u64)
}

/// Exact product of a little-endian limb slice with a small (`<= 2^53`)
/// unsigned multiplier, as a fresh limb vector. The top limb holds the carry,
/// so the product is exact.
fn slice_mul_u64(words: &[u64], multiplier: u64) -> Vec<u64> {
    if multiplier == 0 || slice_bit_len(words) == 0 {
        return vec![0];
    }
    let mut out = vec![0u64; words.len() + 1];
    let mut carry = 0u128;
    for (i, &word) in words.iter().enumerate() {
        let product = (word as u128) * (multiplier as u128) + carry;
        out[i] = product as u64;
        carry = product >> 64;
    }
    out[words.len()] = carry as u64;
    while out.len() > 1 && *out.last().expect("nonempty") == 0 {
        out.pop();
    }
    out
}

/// Compare two little-endian nonnegative magnitudes.
fn slice_cmp(a: &[u64], b: &[u64]) -> Ordering {
    let ab = slice_bit_len(a);
    let bb = slice_bit_len(b);
    if ab != bb {
        return ab.cmp(&bb);
    }
    let words = ab.div_ceil(64);
    for i in (0..words).rev() {
        let av = a.get(i).copied().unwrap_or(0);
        let bv = b.get(i).copied().unwrap_or(0);
        if av != bv {
            return av.cmp(&bv);
        }
    }
    Ordering::Equal
}

/// `a -= b` over little-endian magnitudes; callers guarantee `a >= b`.
fn slice_sub_assign(a: &mut [u64], b: &[u64]) {
    let mut borrow = 0u64;
    for (i, word) in a.iter_mut().enumerate() {
        let bv = b.get(i).copied().unwrap_or(0);
        let (d1, b1) = word.overflowing_sub(bv);
        let (d2, b2) = d1.overflowing_sub(borrow);
        *word = d2;
        borrow = u64::from(b1) + u64::from(b2);
    }
    debug_assert_eq!(borrow, 0, "slice subtraction underflow");
}

/// Exact binary long division of two nonnegative little-endian magnitudes,
/// returning `(quotient, remainder)`.
///
/// The whole quotient is computed (no truncation) so the caller can take the
/// single correctly rounded step at the destination precision. The widths
/// involved are bounded by the 66-word exact-dyadic magnitude, so the
/// bit-serial loop is cheap.
fn slice_div_rem(num: &[u64], den: &[u64]) -> (Vec<u64>, Vec<u64>) {
    let db = slice_bit_len(den);
    assert!(db > 0, "division by zero exact-dyadic magnitude");
    let nb = slice_bit_len(num);
    let mut quotient = vec![0u64; nb.div_ceil(64).max(1)];
    // The running remainder stays strictly below `den`, so `db` bits (plus one
    // for the pre-subtraction doubling) suffice.
    let mut rem = vec![0u64; db.div_ceil(64) + 1];
    for bit in (0..nb).rev() {
        // `rem = rem * 2 + bit(num, bit)`.
        let mut carry = u64::from(slice_bit(num, bit));
        for word in rem.iter_mut() {
            let next = *word >> 63;
            *word = (*word << 1) | carry;
            carry = next;
        }
        if slice_cmp(&rem, den) != Ordering::Less {
            slice_sub_assign(&mut rem, den);
            quotient[bit / 64] |= 1u64 << (bit % 64);
        }
    }
    (quotient, rem)
}

/// Correctly rounded `f64` of the exact rational
/// `(-1)^negative * (num * 2^num_exp) / (den * 2^den_exp)`.
///
/// This is the single-rounding rational division the CORE-W04 exact-`N`
/// inverse and the `delta = delta_num / N` logarithm branch require: the
/// numerator and denominator are never independently rounded before dividing.
/// The numerator is pre-scaled so the integer quotient carries at least 55
/// bits, the division remainder is folded into the sticky bit, and exactly one
/// round-to-nearest-even is taken at the destination precision.
fn exact_division_to_f64(
    num: &[u64],
    num_exp: i32,
    den: &[u64],
    den_exp: i32,
    negative: bool,
) -> f64 {
    let nb = slice_bit_len(num);
    let db = slice_bit_len(den);
    if nb == 0 {
        return 0.0;
    }
    if db == 0 {
        return if negative {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        };
    }
    // 55 quotient bits: 53 significand bits plus guard and sticky headroom.
    const TARGET: i64 = 55;
    let shift = db as i64 + TARGET - nb as i64;
    let (scaled_num, scaled_den, adj_shift) = if shift >= 0 {
        (slice_shl(num, shift as usize), den.to_vec(), shift)
    } else {
        (num.to_vec(), slice_shl(den, (-shift) as usize), shift)
    };
    let (quotient, remainder) = slice_div_rem(&scaled_num, &scaled_den);
    let sticky = remainder.iter().any(|&word| word != 0);
    let e_offset = num_exp as i64 - den_exp as i64 - adj_shift;
    round_words_scaled_to_f64(negative, &quotient, e_offset, sticky)
}

/// Correctly rounded `f64` of `numerator / denominator`, with `denominator` an
/// exact positive dyadic (CORE-W04 R1-F2: the stored-`N` inverse and sandwich
/// divide each coefficient by the exact `N`, never by a pre-rounded `f64`).
///
/// A non-finite numerator is returned unchanged (the exact route cannot
/// represent it); callers that reach this point have already passed the
/// finite-input reverse-norm gate for `inverse`, and preserve the existing
/// overflow semantics for `apply`.
pub(crate) fn divide_f64_by_positive(numerator: f64, denominator: &ExactPositive) -> f64 {
    if numerator == 0.0 {
        return 0.0;
    }
    let Some((mantissa, exponent, negative)) = decompose(numerator) else {
        return numerator;
    };
    let single = [mantissa];
    exact_division_to_f64(
        &single,
        exponent,
        &denominator.magnitude,
        denominator.exponent,
        negative,
    )
}

/// Correctly rounded `f64` of `(-1)^negative * words * 2^e_offset`, where
/// `words` is an exact nonnegative integer and `external_sticky` reports any
/// nonzero bits below the least-significant bit of `words` (used for the
/// remainder of an integer division).
///
/// The single rounding is taken at the destination precision: 53 bits for a
/// normal result and the true subnormal precision otherwise. This is the one
/// rounding decision shared by the symbolic scalar composition and the
/// exact-polynomial finalization, which removes the former
/// normalized-then-subnormal double rounding.
fn round_words_scaled_to_f64(
    negative: bool,
    words: &[u64],
    e_offset: i64,
    external_sticky: bool,
) -> f64 {
    let b = slice_bit_len(words);
    if b == 0 {
        return 0.0;
    }
    let sign = if negative { -1.0 } else { 1.0 };
    let msb_exp = b as i64 - 1 + e_offset;
    if msb_exp > 1023 {
        return sign * f64::INFINITY;
    }
    if msb_exp < -1075 {
        return sign * 0.0;
    }
    if msb_exp == -1075 {
        // Value in `[2^-1075, 2^-1074)`: exactly `2^-1075` ties to even (0),
        // anything strictly larger rounds to the minimum subnormal.
        let rounded_up = slice_has_other_bits(words, b - 1) || external_sticky;
        return if rounded_up {
            sign * f64::from_bits(1)
        } else {
            sign * 0.0
        };
    }
    let keep = if msb_exp >= -1022 {
        53usize
    } else {
        (msb_exp + 1075) as usize
    };
    let shift = b.saturating_sub(keep);
    let retained = slice_value_above(words, shift);
    let (guard, sticky) = if shift > 0 {
        (
            slice_bit(words, shift - 1),
            external_sticky || slice_any_bit_below(words, shift - 1),
        )
    } else {
        (false, external_sticky)
    };
    let mut rounded = retained;
    if guard && (sticky || rounded & 1 == 1) {
        rounded += 1;
    }
    let mut exponent = e_offset + shift as i64;
    if rounded == (1u64 << (b - shift)) {
        rounded >>= 1;
        exponent += 1;
    }
    sign * scale_pow2(rounded as f64, exponent)
}

/// Correctly rounded `f64` of `(-1)^negative * magnitude * 2^exponent /
/// denominator * scale_mantissa * 2^scale_exponent`.
///
/// The scale mantissa is folded exactly into the division numerator, the scale
/// exponent into the final exponent, and the division remainder into the
/// sticky bit, so exactly one rounding is taken at the destination precision.
/// This is the exact-polynomial finalization path (including the symbolic
/// scalar scale); everything before the final rounding is exact integer
/// arithmetic.
fn wide_rational_scaled_to_f64(
    magnitude: &[u64; WIDE_WORDS],
    exponent: i32,
    denominator: u64,
    negative: bool,
    scale_mantissa: f64,
    scale_exponent: i64,
) -> f64 {
    let Some((scale_m, scale_e, scale_negative)) = decompose(scale_mantissa) else {
        return f64::NAN;
    };
    if scale_m == 0 {
        return 0.0;
    }
    let numerator = slice_mul_u64(magnitude, scale_m);
    if slice_bit_len(&numerator) == 0 {
        return 0.0;
    }
    let total_exponent = exponent as i64 + scale_e as i64 + scale_exponent;
    let shifted = slice_shl(&numerator, 256);
    let (quotient, remainder) = slice_div_u64(&shifted, denominator);
    round_words_scaled_to_f64(
        negative ^ scale_negative,
        &quotient,
        total_exponent - 256,
        remainder != 0,
    )
}

/// A signed exact coefficient under construction: the value
/// `(-1)^negative * magnitude * 2^exponent`, with the magnitude held in the
/// wide form. All added terms are aligned to the smallest exponent seen so
/// far.
#[derive(Clone, Copy)]
struct WideAccumulator {
    magnitude: [u64; WIDE_WORDS],
    negative: bool,
    exponent: i32,
    initialized: bool,
}

impl WideAccumulator {
    const fn zero() -> Self {
        Self {
            magnitude: [0u64; WIDE_WORDS],
            negative: false,
            exponent: 0,
            initialized: false,
        }
    }

    /// Add `(-1)^negative * magnitude * 2^exponent`, or return `false` when
    /// aligning the exponents would shift a nonzero bit past the wide width.
    fn add_term(&mut self, magnitude: &[u64; WIDE_WORDS], exponent: i32, negative: bool) -> bool {
        if !self.initialized {
            self.magnitude = *magnitude;
            self.negative = negative;
            self.exponent = exponent;
            self.initialized = true;
            return true;
        }
        if exponent == self.exponent {
            self.combine(magnitude, negative);
            true
        } else if exponent > self.exponent {
            let Some(shifted) = wide_shl(magnitude, (exponent - self.exponent) as usize) else {
                return false;
            };
            self.combine(&shifted, negative);
            true
        } else {
            let Some(shifted) = wide_shl(&self.magnitude, (self.exponent - exponent) as usize)
            else {
                return false;
            };
            self.magnitude = shifted;
            self.exponent = exponent;
            self.combine(magnitude, negative);
            true
        }
    }

    /// Add an already exponent-aligned term of either sign.
    fn combine(&mut self, magnitude: &[u64; WIDE_WORDS], negative: bool) {
        if self.magnitude.iter().all(|&word| word == 0) {
            self.magnitude = *magnitude;
            self.negative = negative;
            return;
        }
        if self.negative == negative {
            wide_add(&mut self.magnitude, magnitude);
        } else {
            match wide_cmp(&self.magnitude, magnitude) {
                Ordering::Greater => wide_sub(&mut self.magnitude, magnitude),
                Ordering::Less => {
                    let mut difference = *magnitude;
                    wide_sub(&mut difference, &self.magnitude);
                    self.magnitude = difference;
                    self.negative = negative;
                }
                Ordering::Equal => {
                    self.magnitude = [0u64; WIDE_WORDS];
                    self.negative = false;
                }
            }
        }
    }

    /// Collapse to an [`ExactDyadic`], or `None` if the accumulated magnitude
    /// does not fit the 66-word representation.
    fn to_exact(self) -> Option<ExactDyadic> {
        if !self.initialized || self.magnitude.iter().all(|&word| word == 0) {
            return Some(ExactDyadic::zero());
        }
        if self.magnitude[WORDS..].iter().any(|&word| word != 0) {
            return None;
        }
        let mut magnitude = [0u64; WORDS];
        magnitude.copy_from_slice(&self.magnitude[..WORDS]);
        Some(ExactDyadic {
            magnitude,
            negative: self.negative,
            exponent: self.exponent,
        })
    }

    /// The exact wide value `(-1)^negative * magnitude * 2^exponent`. An
    /// uninitialised accumulator is the zero value (all-zero magnitude).
    fn parts(self) -> ([u64; WIDE_WORDS], bool, i32) {
        (self.magnitude, self.negative, self.exponent)
    }
}

/// An exact multivector power over the first `basis_count` basis blades.
struct ExactPower {
    coeffs: [ExactDyadic; MAX_BASIS],
    basis_count: usize,
}

impl ExactPower {
    /// The multiplicative identity power `y^0`.
    fn identity<const P: usize, const Q: usize, const R: usize>() -> Self {
        let mut coeffs = [ExactDyadic::zero(); MAX_BASIS];
        let mut one = [0u64; WORDS];
        one[0] = 1;
        coeffs[0] = ExactDyadic {
            magnitude: one,
            negative: false,
            exponent: 0,
        };
        Self {
            coeffs,
            basis_count: Multivector::<P, Q, R>::BASIS_COUNT,
        }
    }

    /// Exact power of a finite multivector's coefficients, or `None` if any
    /// coefficient is non-finite (which the exact route does not support).
    fn from_multivector<const P: usize, const Q: usize, const R: usize>(
        x: &Multivector<P, Q, R>,
    ) -> Option<Self> {
        let basis_count = Multivector::<P, Q, R>::BASIS_COUNT;
        // The exact certification machinery supports at most `MAX_BASIS`
        // coefficients (`n <= 6`); a larger signature is reported as
        // uncertified rather than pretending a partial table is a certificate
        // (CORE-W04 R1-F1).
        if basis_count > MAX_BASIS {
            return None;
        }
        let mut coeffs = [ExactDyadic::zero(); MAX_BASIS];
        for (index, slot) in coeffs.iter_mut().enumerate().take(basis_count) {
            let coefficient = x.get(index);
            if coefficient == 0.0 {
                continue;
            }
            *slot = ExactDyadic::from_f64(coefficient)?;
        }
        Some(Self {
            coeffs,
            basis_count,
        })
    }

    /// Full square-zero certificate for a power.
    fn is_zero(&self) -> bool {
        // `take` rather than a slice so an over-wide `basis_count` can never
        // index past the fixed table (CORE-W04 R1-F1).
        self.coeffs
            .iter()
            .take(self.basis_count)
            .all(ExactDyadic::is_zero)
    }
}

/// Exact product of two powers, or `None` if a coefficient left the 66-word
/// magnitude or `i32` exponent range. The caller treats `None` as "not
/// certified" and routes to the series.
fn mul_exact<const P: usize, const Q: usize, const R: usize>(
    a: &ExactPower,
    b: &ExactPower,
) -> Option<ExactPower> {
    let dim = P + Q + R;
    let basis_count = a.basis_count;
    let mut accumulators = [WideAccumulator::zero(); MAX_BASIS];
    for (i, a_coeff) in a.coeffs.iter().enumerate().take(basis_count) {
        if a_coeff.is_zero() {
            continue;
        }
        for (j, b_coeff) in b.coeffs.iter().enumerate().take(basis_count) {
            if b_coeff.is_zero() {
                continue;
            }
            let (sign, index) = product_sign_and_index(i, j, P, Q, dim);
            if sign == 0 {
                continue;
            }
            let exponent = a_coeff.exponent.checked_add(b_coeff.exponent)?;
            let magnitude = mul_magnitude(&a_coeff.magnitude, &b_coeff.magnitude);
            let negative = a_coeff.negative ^ b_coeff.negative ^ (sign < 0);
            if !accumulators[index].add_term(&magnitude, exponent, negative) {
                return None;
            }
        }
    }
    let mut coeffs = [ExactDyadic::zero(); MAX_BASIS];
    for (index, slot) in coeffs.iter_mut().enumerate().take(basis_count) {
        *slot = accumulators[index].to_exact()?;
    }
    Some(ExactPower {
        coeffs,
        basis_count,
    })
}

/// Exact per-coefficient quotient of the versor sandwich `R·A·R† / N`.
///
/// Every numerator coefficient is the exact dyadic expansion of the stored
/// `R·A·R†` (two exact multivector products), divided by the certified exact
/// positive `N` through the sticky-bit divider and rounded **once** at the
/// destination precision. No `f64` numerator is ever materialized, so a
/// nonzero image whose coefficients underflow is still rounded correctly
/// instead of collapsing to zero (CORE-W04 R3-F14).
///
/// `None` when the exact expansion leaves the representable dyadic window (the
/// caller then falls back to the floating sandwich).
pub(crate) fn exact_sandwich_divide<const P: usize, const Q: usize, const R: usize>(
    rotor: &Multivector<P, Q, R>,
    v: &Multivector<P, Q, R>,
    n: &ExactPositive,
) -> Option<Multivector<P, Q, R>> {
    if Multivector::<P, Q, R>::BASIS_COUNT > MAX_BASIS {
        return None;
    }
    let left = ExactPower::from_multivector(rotor)?;
    let middle = ExactPower::from_multivector(v)?;
    let reversed = rotor.reverse();
    let right = ExactPower::from_multivector(&reversed)?;
    let first = mul_exact::<P, Q, R>(&left, &middle)?;
    let sandwich = mul_exact::<P, Q, R>(&first, &right)?;
    let mut out = Multivector::<P, Q, R>::zero();
    for (index, coefficient) in sandwich
        .coeffs
        .iter()
        .enumerate()
        .take(sandwich.basis_count)
    {
        if coefficient.is_zero() {
            continue;
        }
        // `N` is strictly positive, so no sign or zero gate is needed here:
        // the single sticky-bit division rounds at the destination.
        let signed = ExactSigned {
            magnitude: coefficient.magnitude,
            exponent: coefficient.exponent,
            negative: coefficient.negative,
        };
        out.set(index, signed.ratio_to_f64(n));
    }
    Some(out)
}

/// Outcome of the exact reverse-norm certificate `N = R·R†` (CORE-W04 §1.4b).
///
/// `Positive` carries the certified scalar `N` rounded once to `f64`
/// (round-to-nearest-even). The other variants are the distinct rejection
/// classes the rotor gate must report as [`crate::RotorError::Degenerate`].
pub(crate) enum ReverseNorm {
    /// `R·R†` is exactly a positive scalar.
    Positive,
    /// `R·R†` has a nonzero non-scalar part.
    NonScalar,
    /// `R·R†` is exactly a scalar but is `<= 0` (including the zero rotor).
    NonPositive,
    /// The exact product left the representable dyadic window, or an input
    /// coefficient was non-finite.
    Uncertified,
}

/// Exact certification of the scalar reverse norm `N = R·R†`.
///
/// Uses the W03 exact dyadic product machinery ([`ExactPower`] + [`mul_exact`])
/// rather than a floating-point scalar product, so near-cancelling cases such as
/// `w = β = 1e154` and the finite `R^32 = [2³¹, 2³¹]` chain (`N = 0` exactly)
/// are classified without overflow or rounding. The scalar is rounded once
/// through [`round_words_scaled_to_f64`].
pub(crate) fn exact_reverse_norm<const P: usize, const Q: usize, const R: usize>(
    r: &Multivector<P, Q, R>,
) -> ReverseNorm {
    if Multivector::<P, Q, R>::BASIS_COUNT > MAX_BASIS {
        return ReverseNorm::Uncertified;
    }
    let Some(left) = ExactPower::from_multivector(r) else {
        return ReverseNorm::Uncertified;
    };
    let reversed = r.reverse();
    let Some(right) = ExactPower::from_multivector(&reversed) else {
        return ReverseNorm::Uncertified;
    };
    let Some(product) = mul_exact::<P, Q, R>(&left, &right) else {
        return ReverseNorm::Uncertified;
    };
    for coefficient in product.coeffs.iter().take(product.basis_count).skip(1) {
        if !coefficient.is_zero() {
            return ReverseNorm::NonScalar;
        }
    }
    let scalar = product.coeffs[0];
    if scalar.is_zero() || scalar.negative {
        return ReverseNorm::NonPositive;
    }
    ReverseNorm::Positive
}

// ======================================================================
// CORE-W04 §2c range-safe logarithm scalars.
//
// The domain-aware rotor logarithm evaluates every scalar stage on exact
// dyadic rationals and only rounds once at the destination precision, via the
// symbolic mantissa/exponent decomposition `value = significand * 2^e` with
// `significand in [1, 2)`. These helpers expose the W03 dyadic machinery as an
// exact positive dyadic (`ExactPositive`), an exact signed dyadic
// (`ExactSigned`), exact add/mul/sub, an exact magnitude comparison, and a
// range-safe `ln`. Direct `f64` evaluation of `w^2 + beta^2` (which overflows
// for `w = beta = 1e154`) is thereby impossible.
// ======================================================================

/// An exact positive dyadic scalar `magnitude * 2^exponent`.
#[derive(Clone, Copy)]
pub(crate) struct ExactPositive {
    magnitude: [u64; WORDS],
    exponent: i32,
}

/// An exact signed dyadic scalar `(-1)^negative * magnitude * 2^exponent`.
#[derive(Clone, Copy)]
pub(crate) struct ExactSigned {
    magnitude: [u64; WORDS],
    exponent: i32,
    negative: bool,
}

/// Widen a 66-word magnitude into the wide accumulator form.
fn to_wide(words: &[u64; WORDS]) -> [u64; WIDE_WORDS] {
    let mut out = [0u64; WIDE_WORDS];
    out[..WORDS].copy_from_slice(words);
    out
}

/// Correctly rounded significand in `[1, 2)` and the binary exponent `e` of an
/// exact dyadic `magnitude * 2^exponent`, so the value is
/// `significand * 2^e`.
fn normalized_dyadic(magnitude: &[u64; WORDS], exponent: i32) -> (f64, i64) {
    let bits = slice_bit_len(magnitude);
    let (top, sticky) = top_128(magnitude, bits - 1);
    let (significand, carry) = round_mantissa_to_f64(top, sticky);
    (
        significand,
        exponent as i64 + bits as i64 - 1 + i64::from(carry),
    )
}

impl ExactPositive {
    /// Exact value of a finite, strictly positive `f64`.
    pub(crate) fn from_f64(c: f64) -> Option<Self> {
        if !c.is_finite() || c <= 0.0 {
            return None;
        }
        let (mantissa, exponent, _) = decompose(c)?;
        if mantissa == 0 {
            return None;
        }
        let mut magnitude = [0u64; WORDS];
        magnitude[0] = mantissa;
        Some(Self {
            magnitude,
            exponent,
        })
    }

    /// Exact product, or `None` when the magnitude leaves the 66-word window.
    pub(crate) fn mul(&self, other: &Self) -> Option<Self> {
        let product = mul_magnitude(&self.magnitude, &other.magnitude);
        if product[WORDS..].iter().any(|&word| word != 0) {
            return None;
        }
        let mut magnitude = [0u64; WORDS];
        magnitude.copy_from_slice(&product[..WORDS]);
        let exponent = self.exponent.checked_add(other.exponent)?;
        Some(Self {
            magnitude,
            exponent,
        })
    }

    /// Exact sum of two positive dyadics.
    pub(crate) fn add(&self, other: &Self) -> Option<Self> {
        let mut accumulator = WideAccumulator::zero();
        if !accumulator.add_term(&to_wide(&self.magnitude), self.exponent, false) {
            return None;
        }
        if !accumulator.add_term(&to_wide(&other.magnitude), other.exponent, false) {
            return None;
        }
        let dyadic = accumulator.to_exact()?;
        if dyadic.negative || dyadic.is_zero() {
            return None;
        }
        Some(Self {
            magnitude: dyadic.magnitude,
            exponent: dyadic.exponent,
        })
    }

    /// Exact `self - other` as a signed dyadic.
    pub(crate) fn sub(&self, other: &Self) -> Option<ExactSigned> {
        let mut accumulator = WideAccumulator::zero();
        if !accumulator.add_term(&to_wide(&self.magnitude), self.exponent, false) {
            return None;
        }
        if !accumulator.add_term(&to_wide(&other.magnitude), other.exponent, true) {
            return None;
        }
        let dyadic = accumulator.to_exact()?;
        Some(ExactSigned {
            magnitude: dyadic.magnitude,
            exponent: dyadic.exponent,
            negative: dyadic.negative,
        })
    }

    /// Range-safe natural logarithm `ln(m) + e·ln 2` with `m in [1, 2)`.
    pub(crate) fn ln(&self) -> f64 {
        let (significand, exponent) = normalized_dyadic(&self.magnitude, self.exponent);
        significand.ln() + (exponent as f64) * core::f64::consts::LN_2
    }

    /// Range-safe half logarithm `½·ln(self)`.
    pub(crate) fn ln_half(&self) -> f64 {
        0.5 * self.ln()
    }

    /// Range-safe `ln(self / denominator)`, combining the two normalized
    /// significands with a single shared power-of-two exponent term.
    ///
    /// Compared with `self.ln() - denominator.ln()`, this avoids one rounding
    /// of the dominant `e·ln 2` term and is what makes the R8 round-trip
    /// witness's rapidity correctly rounded.
    pub(crate) fn ln_ratio(&self, denominator: &Self) -> f64 {
        let (num_significand, num_exponent) = normalized_dyadic(&self.magnitude, self.exponent);
        let (den_significand, den_exponent) =
            normalized_dyadic(&denominator.magnitude, denominator.exponent);
        num_significand.ln() - den_significand.ln()
            + ((num_exponent - den_exponent) as f64) * core::f64::consts::LN_2
    }
}

impl ExactSigned {
    /// Quotient `self / denominator` as one correctly rounded `f64`.
    ///
    /// CORE-W04 R1-F3: the quotient is formed from the exact dyadic
    /// magnitudes by a single long division with the remainder folded into the
    /// sticky bit, so the result is correctly rounded once (the previous
    /// "normalize each significand, then divide" double-rounded and was 2-3 ulp
    /// off for the `delta = delta_num / N` rapidity branch).
    pub(crate) fn ratio_to_f64(&self, denominator: &ExactPositive) -> f64 {
        exact_division_to_f64(
            &self.magnitude,
            self.exponent,
            &denominator.magnitude,
            denominator.exponent,
            self.negative,
        )
    }

    /// Exact magnitude comparison `|self|` vs `other`.
    pub(crate) fn cmp_abs(&self, other: &ExactPositive) -> Option<Ordering> {
        let mut accumulator = WideAccumulator::zero();
        if !accumulator.add_term(&to_wide(&self.magnitude), self.exponent, self.negative) {
            return None;
        }
        if !accumulator.add_term(&to_wide(&other.magnitude), other.exponent, true) {
            return None;
        }
        let (magnitude, negative, _) = accumulator.parts();
        Some(signed_order(&magnitude, negative))
    }
}

/// Order of a wide signed magnitude (`negative` is the sign flag).
fn signed_order(magnitude: &[u64; WIDE_WORDS], negative: bool) -> Ordering {
    if magnitude.iter().all(|&word| word == 0) {
        Ordering::Equal
    } else if negative {
        Ordering::Less
    } else {
        Ordering::Greater
    }
}

/// Exact scalar reverse norm `N = R·R†` when it is exactly a positive scalar,
/// returned as an exact dyadic for the CORE-W04 hyperbolic logarithm.
///
/// `None` for a non-scalar product, a `<= 0` scalar, or an uncertified
/// product; the logarithm maps every `None` to an error.
pub(crate) fn exact_reverse_norm_positive<const P: usize, const Q: usize, const R: usize>(
    r: &Multivector<P, Q, R>,
) -> Option<ExactPositive> {
    if Multivector::<P, Q, R>::BASIS_COUNT > MAX_BASIS {
        return None;
    }
    let left = ExactPower::from_multivector(r)?;
    let reversed = r.reverse();
    let right = ExactPower::from_multivector(&reversed)?;
    let product = mul_exact::<P, Q, R>(&left, &right)?;
    for coefficient in product.coeffs.iter().take(product.basis_count).skip(1) {
        if !coefficient.is_zero() {
            return None;
        }
    }
    let scalar = product.coeffs[0];
    if scalar.is_zero() || scalar.negative {
        return None;
    }
    Some(ExactPositive {
        magnitude: scalar.magnitude,
        exponent: scalar.exponent,
    })
}

/// Sign of the exact scalar part of `N = R·R†`, ignoring any non-scalar
/// rounding residue.
///
/// Superseded by the full positive-scalar certificate
/// [`exact_reverse_norm`] at the CORE-W04 binary-power boundary (R1-F5): a
/// multi-plane product whose exact `R·R†` has non-scalar content must be
/// rejected, not accepted on the sign of the scalar part alone. Retained as
/// the documented previous gate.
#[allow(dead_code)]
pub(crate) enum ReverseNormScalarSign {
    /// The exact scalar part is strictly positive.
    Positive,
    /// The exact scalar part is exactly zero or negative.
    NonPositive,
    /// The exact product left the representable dyadic window, or an input
    /// coefficient was non-finite.
    Uncertified,
}

/// Sign classification of the exact scalar part of `N = R·R†` (superseded by
/// [`exact_reverse_norm`]; see [`ReverseNormScalarSign`]).
#[allow(dead_code)]
pub(crate) fn exact_reverse_norm_scalar_sign<const P: usize, const Q: usize, const R: usize>(
    r: &Multivector<P, Q, R>,
) -> ReverseNormScalarSign {
    if Multivector::<P, Q, R>::BASIS_COUNT > MAX_BASIS {
        return ReverseNormScalarSign::Uncertified;
    }
    let Some(left) = ExactPower::from_multivector(r) else {
        return ReverseNormScalarSign::Uncertified;
    };
    let reversed = r.reverse();
    let Some(right) = ExactPower::from_multivector(&reversed) else {
        return ReverseNormScalarSign::Uncertified;
    };
    let Some(product) = mul_exact::<P, Q, R>(&left, &right) else {
        return ReverseNormScalarSign::Uncertified;
    };
    let scalar = product.coeffs[0];
    if scalar.negative || scalar.is_zero() {
        ReverseNormScalarSign::NonPositive
    } else {
        ReverseNormScalarSign::Positive
    }
}

/// Exact sign/value classification of the scalar `N = R·R†`.
///
/// Unlike [`ReverseNorm`], this distinguishes the exact scalar zero from an
/// exact negative scalar (CORE-W04 §3a candidate trichotomy: `CC† = 0` ->
/// [`crate::RotorError::NullCandidate`], `CC† < 0` ->
/// [`crate::RotorError::SheetChange`]).
pub(crate) enum ReverseNormState {
    /// `R·R†` is exactly a positive scalar.
    Positive,
    /// `R·R†` is exactly the scalar zero.
    Zero,
    /// `R·R†` is exactly a negative scalar.
    Negative,
    /// `R·R†` has a nonzero non-scalar part.
    NonScalar,
    /// The exact product left the representable dyadic window, or an input
    /// coefficient was non-finite.
    Uncertified,
}

/// Exact sign/value classification of `N = R·R†` (see [`ReverseNormState`]).
pub(crate) fn exact_reverse_norm_state<const P: usize, const Q: usize, const R: usize>(
    r: &Multivector<P, Q, R>,
) -> ReverseNormState {
    if Multivector::<P, Q, R>::BASIS_COUNT > MAX_BASIS {
        return ReverseNormState::Uncertified;
    }
    let Some(left) = ExactPower::from_multivector(r) else {
        return ReverseNormState::Uncertified;
    };
    let reversed = r.reverse();
    let Some(right) = ExactPower::from_multivector(&reversed) else {
        return ReverseNormState::Uncertified;
    };
    let Some(product) = mul_exact::<P, Q, R>(&left, &right) else {
        return ReverseNormState::Uncertified;
    };
    for coefficient in product.coeffs.iter().take(product.basis_count).skip(1) {
        if !coefficient.is_zero() {
            return ReverseNormState::NonScalar;
        }
    }
    let scalar = product.coeffs[0];
    if scalar.is_zero() {
        return ReverseNormState::Zero;
    }
    if scalar.negative {
        return ReverseNormState::Negative;
    }
    let value = round_words_scaled_to_f64(false, &scalar.magnitude, scalar.exponent as i64, false);
    if value.is_finite() && value > 0.0 {
        ReverseNormState::Positive
    } else {
        ReverseNormState::Uncertified
    }
}

/// Exact scalar `N = R·R†` when it is exactly `+1` or `-1`; `None` otherwise
/// (non-scalar, non-finite, uncertified, or any other value).
///
/// CORE-W04 §3b: a reflection normal must be a certified pure vector with
/// `n·n = ±1` exactly; a rounded `sqrt` check cannot certify exactness, so the
/// exact dyadic magnitude is compared against the exact unit.
pub(crate) fn exact_reverse_norm_unit<const P: usize, const Q: usize, const R: usize>(
    r: &Multivector<P, Q, R>,
) -> Option<f64> {
    if Multivector::<P, Q, R>::BASIS_COUNT > MAX_BASIS {
        return None;
    }
    let left = ExactPower::from_multivector(r)?;
    let reversed = r.reverse();
    let right = ExactPower::from_multivector(&reversed)?;
    let product = mul_exact::<P, Q, R>(&left, &right)?;
    for coefficient in product.coeffs.iter().take(product.basis_count).skip(1) {
        if !coefficient.is_zero() {
            return None;
        }
    }
    let scalar = product.coeffs[0];
    if scalar.is_zero() {
        return None;
    }
    // Exactly `±1` iff the exact magnitude is a single power of two `2^b` and
    // `b + exponent == 0`.
    let mut set_bit: Option<usize> = None;
    for (word_index, &word) in scalar.magnitude.iter().enumerate() {
        if word == 0 {
            continue;
        }
        if word.count_ones() != 1 || set_bit.is_some() {
            return None;
        }
        set_bit = Some(word.trailing_zeros() as usize + word_index * 64);
    }
    let bit = set_bit?;
    if scalar.exponent as i64 + bit as i64 != 0 {
        return None;
    }
    Some(if scalar.negative { -1.0 } else { 1.0 })
}

/// Exact equality of the two dyadic products `x1·y1` and `x2·y2`.
///
/// Used by the CORE-W04 §3a direction pre-test: every finite `f64` is
/// `(-1)^n · m · 2^e` with a 53-bit integer `m`, so each product is the exact
/// 106-bit dyadic `(-1)^(n1^n2) · (m1·m2) · 2^(e1+e2)`. Canonicalizing each
/// product by stripping its power-of-two factor makes the comparison exact and
/// overflow-free. `false` for any non-finite input.
pub(crate) fn exact_product_eq(x1: f64, y1: f64, x2: f64, y2: f64) -> bool {
    let (Some((m1, e1, n1)), Some((m2, e2, n2))) = (decompose(x1), decompose(y1)) else {
        return false;
    };
    let (Some((m3, e3, n3)), Some((m4, e4, n4))) = (decompose(x2), decompose(y2)) else {
        return false;
    };
    let p_zero = m1 == 0 || m2 == 0;
    let q_zero = m3 == 0 || m4 == 0;
    if p_zero || q_zero {
        return p_zero && q_zero;
    }
    if (n1 ^ n2) != (n3 ^ n4) {
        return false;
    }
    let p = (m1 as u128) * (m2 as u128);
    let q = (m3 as u128) * (m4 as u128);
    let p_tz = p.trailing_zeros();
    let q_tz = q.trailing_zeros();
    let p_odd = p >> p_tz;
    let q_odd = q >> q_tz;
    let p_exp = e1 as i64 + e2 as i64 + p_tz as i64;
    let q_exp = e3 as i64 + e4 as i64 + q_tz as i64;
    p_odd == q_odd && p_exp == q_exp
}

pub(crate) enum NilpotentOutcome<const P: usize, const Q: usize, const R: usize> {
    /// `y` is nilpotent of index `d <= 16`; this is the exact polynomial
    /// `exp(y) = sum_{j<d} y^j / j!` rounded to `f64`.
    Exp(Multivector<P, Q, R>),
    /// `y` is not certified nilpotent within the 16-power cap (or an exact
    /// coefficient left the representable range). The caller routes to the
    /// bounded series.
    NotNilpotent,
    /// The exact polynomial was computed but a coefficient is not a finite
    /// `f64` even before the symbolic scale; the caller reports
    /// `NonFiniteResult { stage: ClosedForm }` (the unscaled closed form is
    /// itself unrepresentable).
    NonFiniteClosedForm,
    /// The exact unscaled polynomial coefficient was finite, but the symbolic
    /// scalar scale pushed it out of range; the caller reports
    /// `NonFiniteResult { stage: ScalarRoute }` (round-4 stage distinction).
    NonFiniteScalarRoute,
}

/// Maximum number of exact powers evaluated before the nilpotency test gives
/// up. A nilpotent element `y` of a semisimple quotient/radical decomposition
/// in a signature with `dim <= 6` has index at most 12 (the radical contributes
/// `ceil(dim/2) <= 3` and the semisimple part's linear-operator nilpotency adds
/// a bounded number of powers); 16 is documented as `>=` that bound with
/// margin and still bounds the work. The index-8 `Cl(3,3,0)` example below is
/// the pinned worst observed case for `dim = 6`, and the old "at most 7"
/// comment was simply false.
const NILPOTENT_POWER_CAP: usize = 16;

/// `16! = 20_922_789_888_000` clears every factorial `j!` for
/// `j <= NILPOTENT_POWER_CAP` (`j!` divides `16!` for `j <= 16`), so the
/// polynomial `sum_{j<d} y^j / j!` is accumulated as the single exact integer
/// `N_blade = sum_j y^j[blade] * (16! / j!)` and divided by `16!` once, at the
/// end. (The plan text calls this `lcm(1..d)`; `lcm` does not clear `j!` for
/// `j >= 4`, so the faithful denominator is `d!`, fixed here at the cap.)
const NILPOTENT_DENOMINATOR: u64 = 20_922_789_888_000;

/// `j!` for `j = 0..=NILPOTENT_POWER_CAP` (last entry equals
/// [`NILPOTENT_DENOMINATOR`]).
const NILPOTENT_FACTORIALS: [u64; NILPOTENT_POWER_CAP + 1] = [
    1,
    1,
    2,
    6,
    24,
    120,
    720,
    5_040,
    40_320,
    362_880,
    3_628_800,
    39_916_800,
    479_001_600,
    6_227_020_800,
    87_178_291_200,
    1_307_674_368_000,
    20_922_789_888_000,
];

/// Add `power * multiplier` (exact integer scaling) into the per-blade exact
/// accumulators, or return `false` when aligning an exponent would overflow the
/// wide accumulator.
fn accumulate_power_into(
    accumulators: &mut [WideAccumulator; MAX_BASIS],
    power: &ExactPower,
    multiplier: u64,
    basis_count: usize,
) -> bool {
    for (index, accumulator) in accumulators.iter_mut().enumerate().take(basis_count) {
        let coefficient = &power.coeffs[index];
        if coefficient.is_zero() {
            continue;
        }
        let magnitude = mul_magnitude_by_u64(&coefficient.magnitude, multiplier);
        if !accumulator.add_term(&magnitude, coefficient.exponent, coefficient.negative) {
            return false;
        }
    }
    true
}

/// Exact `exp(y)` for nilpotent `y` (precedence 3c).
///
/// Iterates `y^(j+1) = y^j · y` with the exact power product until the
/// all-zero certificate (nilpotency index `d <= NILPOTENT_POWER_CAP`). The
/// polynomial is accumulated **coefficientwise in exact integer arithmetic**:
/// `N_blade = sum_j y^j[blade] * (16! / j!)` (each term an exact bignum times
/// a small integer), and a single round-to-nearest-even conversion of
/// `N_blade / 16!` produces the result. No `f64` exists anywhere between the
/// certification and that one final rounding, so near-cancelling coefficients
/// (the crafted `a = nextafter(1e10)` case) survive exactly.
///
/// The certification iteration never converts to floating point: a power that
/// leaves the exact 66-word magnitude or `i32` exponent range, or an
/// accumulator alignment overflow, returns [`NilpotentOutcome::NotNilpotent`]
/// (the caller falls through to the residual series). The cap being reached
/// without a zero certificate is also `NotNilpotent`. Only a **certified**
/// polynomial whose final (post-division) coefficient is a non-finite `f64`
/// reports [`NilpotentOutcome::NonFiniteClosedForm`] (unscaled unrepresentable)
/// or [`NilpotentOutcome::NonFiniteScalarRoute`] (scale-induced overflow).
#[cfg(test)]
pub(crate) fn exact_nilpotent_exp<const P: usize, const Q: usize, const R: usize>(
    y: &Multivector<P, Q, R>,
) -> NilpotentOutcome<P, Q, R> {
    exact_nilpotent_exp_scaled(y, 1.0, 0)
}

/// Exact `exp(y)` for nilpotent `y`, with the symbolic scalar scale
/// `scale_mantissa * 2^scale_exponent` composed into the single final rounding
/// of each coefficient (the round-4 fix for finite finals whose unscaled
/// polynomial coefficients overflow).
pub(crate) fn exact_nilpotent_exp_scaled<const P: usize, const Q: usize, const R: usize>(
    y: &Multivector<P, Q, R>,
    scale_mantissa: f64,
    scale_exponent: i64,
) -> NilpotentOutcome<P, Q, R> {
    if Multivector::<P, Q, R>::BASIS_COUNT > MAX_BASIS {
        return NilpotentOutcome::NotNilpotent;
    }
    let Some(seed) = ExactPower::from_multivector(y) else {
        return NilpotentOutcome::NotNilpotent;
    };
    let basis_count = seed.basis_count;
    let mut accumulators = [WideAccumulator::zero(); MAX_BASIS];

    // j = 0 term: the identity scaled by the common denominator.
    let mut power = ExactPower::identity::<P, Q, R>();
    if !accumulate_power_into(
        &mut accumulators,
        &power,
        NILPOTENT_DENOMINATOR,
        basis_count,
    ) {
        return NilpotentOutcome::NotNilpotent;
    }

    for &factorial in NILPOTENT_FACTORIALS
        .iter()
        .take(NILPOTENT_POWER_CAP + 1)
        .skip(1)
    {
        let Some(next) = mul_exact::<P, Q, R>(&power, &seed) else {
            return NilpotentOutcome::NotNilpotent;
        };
        power = next;
        if power.is_zero() {
            return finalize_nilpotent::<P, Q, R>(
                &accumulators,
                basis_count,
                scale_mantissa,
                scale_exponent,
            );
        }
        let multiplier = NILPOTENT_DENOMINATOR / factorial;
        if !accumulate_power_into(&mut accumulators, &power, multiplier, basis_count) {
            return NilpotentOutcome::NotNilpotent;
        }
    }
    NilpotentOutcome::NotNilpotent
}

/// Convert the certified exact polynomial accumulator into `f64` coefficients
/// with one round-to-nearest-even per coefficient. A non-finite final value is
/// classified by round-5 stage: the unscaled coefficient is re-rounded without
/// the symbolic scale; if that is finite, only the scale pushed the result out
/// of range (`NonFiniteScalarRoute`), otherwise the unscaled closed form is
/// itself unrepresentable (`NonFiniteClosedForm`).
fn finalize_nilpotent<const P: usize, const Q: usize, const R: usize>(
    accumulators: &[WideAccumulator; MAX_BASIS],
    basis_count: usize,
    scale_mantissa: f64,
    scale_exponent: i64,
) -> NilpotentOutcome<P, Q, R> {
    let mut result = Multivector::<P, Q, R>::zero();
    let mut scale_induced = false;
    for (index, accumulator) in accumulators.iter().enumerate().take(basis_count) {
        let (magnitude, negative, exponent) = accumulator.parts();
        let value = wide_rational_scaled_to_f64(
            &magnitude,
            exponent,
            NILPOTENT_DENOMINATOR,
            negative,
            scale_mantissa,
            scale_exponent,
        );
        if !value.is_finite() {
            let unscaled = wide_rational_scaled_to_f64(
                &magnitude,
                exponent,
                NILPOTENT_DENOMINATOR,
                negative,
                1.0,
                0,
            );
            if !unscaled.is_finite() {
                return NilpotentOutcome::NonFiniteClosedForm;
            }
            scale_induced = true;
        }
        result.set(index, value);
    }
    if scale_induced {
        return NilpotentOutcome::NonFiniteScalarRoute;
    }
    NilpotentOutcome::Exp(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CayleyTable;
    use num_bigint::{BigInt, BigUint, Sign};
    use num_traits::Zero;

    type Mv<const P: usize, const Q: usize, const R: usize> = Multivector<P, Q, R>;

    /// Independent reference: the exact square scaled by `2^2148`, computed with
    /// `num-bigint` over the algebra's full Cayley product. Entry `i` is the
    /// coefficient of blade `i` in `B^2 * 2^2148`.
    fn oracle_scaled_square<const P: usize, const Q: usize, const R: usize>(
        b: &Mv<P, Q, R>,
    ) -> Vec<BigInt> {
        let table = CayleyTable::<P, Q, R>::new();
        let n = Mv::<P, Q, R>::BASIS_COUNT;
        let mut scaled: Vec<BigInt> = vec![BigInt::zero(); n];
        for (i, slot) in scaled.iter_mut().enumerate() {
            let c = b.get(i);
            if c == 0.0 || !c.is_finite() {
                continue;
            }
            let (m, e, negative) = decompose(c).expect("finite");
            let mut value = BigInt::from(m) << ((e + 1074) as usize);
            if negative {
                value = -value;
            }
            *slot = value;
        }

        let mut out: Vec<BigInt> = vec![BigInt::zero(); n];
        for i in 0..n {
            if scaled[i].is_zero() {
                continue;
            }
            for j in 0..n {
                if scaled[j].is_zero() {
                    continue;
                }
                let (sign, index) = table.get_product(i, j);
                if sign == 0.0 {
                    continue;
                }
                let term = &scaled[i] * &scaled[j];
                if sign < 0.0 {
                    out[index] -= &term;
                } else {
                    out[index] += &term;
                }
            }
        }
        out
    }

    /// The accumulator's signed integer value as a `BigInt`.
    fn accumulator_to_bigint(acc: &SignedAccumulator) -> BigInt {
        let magnitude = acc.magnitude();
        let mut limbs = Vec::with_capacity(WORDS * 2);
        for &word in magnitude.iter() {
            limbs.push(word as u32);
            limbs.push((word >> 32) as u32);
        }
        let unsigned = BigUint::from_slice(&limbs);
        if acc.sign() < 0 {
            -BigInt::from(unsigned)
        } else {
            BigInt::from(unsigned)
        }
    }

    /// `BigUint` of a little-endian `u64` limb slice.
    fn words_to_biguint(words: &[u64]) -> BigUint {
        let mut limbs = Vec::with_capacity(words.len() * 2);
        for &word in words {
            limbs.push(word as u32);
            limbs.push((word >> 32) as u32);
        }
        BigUint::from_slice(&limbs)
    }

    /// `(value * 2^2148)` of an exact power coefficient, as a `BigInt`.
    ///
    /// Test inputs use exponents `>= -2148`; the shift is then non-negative
    /// and the exact value times `2^2148` is an integer, directly comparable
    /// with [`oracle_scaled_square`].
    fn exact_dyadic_to_scaled_bigint(c: &ExactDyadic) -> BigInt {
        if c.is_zero() {
            return BigInt::zero();
        }
        let shift = c.exponent + BASE_EXP.abs();
        assert!(shift >= 0, "test exponent {shift} below the oracle base");
        let value = BigInt::from(words_to_biguint(&c.magnitude)) << (shift as usize);
        if c.negative {
            -value
        } else {
            value
        }
    }

    /// Independent reference extraction from a non-negative `BigUint`.
    fn oracle_extract(value: &BigUint) -> Option<(u128, usize, bool)> {
        let bits = value.bits();
        if bits == 0 {
            return None;
        }
        let b = (bits - 1) as usize;
        let mut top = 0u128;
        for k in 0..128 {
            if k > b {
                break;
            }
            if value.bit((b - k) as u64) {
                top |= 1u128 << (127 - k);
            }
        }
        let mut sticky = false;
        if b >= 127 {
            let below = b - 127;
            if below > 0 {
                let mask = (BigUint::from(1u32) << below) - BigUint::from(1u32);
                sticky = !(value & mask).is_zero();
            }
        }
        Some((top, b, sticky))
    }

    /// Check an [`ExactSquare`] against the independent big-integer oracle.
    fn assert_matches_oracle<const P: usize, const Q: usize, const R: usize>(x: &Mv<P, Q, R>) {
        let square = exact_x2(x);
        let oracle = oracle_scaled_square(x);

        let oracle_is_zero = oracle.iter().all(|v| v.is_zero());
        let oracle_exactly_scalar = oracle
            .iter()
            .enumerate()
            .all(|(i, v)| i == 0 || v.is_zero());
        let oracle_grade4_zero = oracle
            .iter()
            .enumerate()
            .all(|(i, v)| i.count_ones() != 4 || v.is_zero());

        assert_eq!(square.is_zero(), oracle_is_zero, "square-zero mismatch");
        assert_eq!(
            square.is_exactly_scalar(),
            oracle_exactly_scalar,
            "scalar-square mismatch"
        );
        assert_eq!(
            square.grade4_is_zero(),
            oracle_grade4_zero,
            "simplicity mismatch"
        );

        // Every output coefficient, exactly (both sides scaled by 2^2148).
        for (index, reference) in oracle.iter().enumerate() {
            assert_eq!(
                accumulator_to_bigint(&square.coeffs[index]),
                *reference,
                "coefficient {index} mismatch"
            );
        }

        let oracle_scalar = &oracle[0];
        let oracle_sign = if oracle_scalar.is_zero() {
            0
        } else if oracle_scalar.sign() == Sign::Minus {
            -1
        } else {
            1
        };
        assert_eq!(square.coeffs[0].sign(), oracle_sign, "scalar sign mismatch");

        let our_extract = square.coeffs[0].extracted();
        let reference = oracle_scalar.magnitude();
        let reference = oracle_extract(reference);
        assert_eq!(our_extract, reference, "extraction mismatch");
    }

    fn mv<const P: usize, const Q: usize, const R: usize>(parts: &[(f64, usize)]) -> Mv<P, Q, R> {
        let mut out = Mv::<P, Q, R>::zero();
        for &(c, index) in parts {
            out.set(index, c);
        }
        out
    }

    fn blade(generators: &[usize]) -> usize {
        let mut index = 0;
        for &g in generators {
            index |= 1 << (g - 1);
        }
        index
    }

    /// The grade-2 basis blade indices of a signature.
    fn bivector_blades(dim: usize) -> Vec<usize> {
        (0..(1usize << dim))
            .filter(|i| i.count_ones() == 2)
            .collect()
    }

    // ------------------------------------------------------------------
    // (b) deterministic mixed-extreme capacity case
    // ------------------------------------------------------------------

    #[test]
    fn mixed_extreme_capacity_is_exact() {
        // H = max finite on e12, L = min subnormal on e13, Cl(3,0,0):
        // scalar = -(H^2 + L^2), an aligned integer with 4196 magnitude bits.
        let h = f64::MAX;
        let l = f64::from_bits(1);
        let b = mv::<3, 0, 0>(&[(h, blade(&[1, 2])), (l, blade(&[1, 3]))]);
        let square = exact_bivector_square(&b);

        assert!(!square.is_zero());
        assert!(square.grade4_is_zero());
        let (sign, angle) = square.scalar_sign_and_angle();
        assert_eq!(sign, -1, "sum of two negative-eta squares is negative");
        assert!(angle.is_finite());

        let magnitude = square.coeffs[0].magnitude();
        assert_eq!(
            bit_len(&magnitude),
            4196,
            "capacity case must occupy the proved 4196-bit span"
        );
        assert_matches_oracle(&b);
    }

    // ------------------------------------------------------------------
    // (c) exhaustive small-signature bivectors vs num-bigint
    // ------------------------------------------------------------------

    fn exhaustive<const P: usize, const Q: usize, const R: usize>() {
        let blades = bivector_blades(P + Q + R);
        let values = [-2.0f64, -0.5, 0.0, 1.5];
        // 1, 3 or 6 bivector blades; for >4 blades this test would blow up, so
        // the enumerated signatures keep the branching below explicit.
        match blades.len() {
            1 => {
                for &a in &values {
                    assert_matches_oracle(&mv::<P, Q, R>(&[(a, blades[0])]));
                }
            }
            3 => {
                for &a in &values {
                    for &b in &values {
                        for &c in &values {
                            assert_matches_oracle(&mv::<P, Q, R>(&[
                                (a, blades[0]),
                                (b, blades[1]),
                                (c, blades[2]),
                            ]));
                        }
                    }
                }
            }
            n => panic!("unexpected bivector count {n}"),
        }
    }

    #[test]
    fn exhaustive_small_signatures() {
        exhaustive::<2, 0, 0>();
        exhaustive::<2, 1, 0>();
        exhaustive::<1, 1, 1>();
        exhaustive::<1, 0, 1>();
    }

    // ------------------------------------------------------------------
    // (a) randomized coefficients vs num-bigint
    // ------------------------------------------------------------------

    struct XorShift(u64);

    impl XorShift {
        fn next(&mut self) -> u64 {
            let mut x = self.0;
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            self.0 = x;
            x
        }

        /// A random finite `f64` spanning the full subnormal..max exponent range.
        fn coeff(&mut self) -> f64 {
            let bits = self.next();
            let sign = bits & (1u64 << 63);
            let biased = ((bits >> 52) & 0x7FF) % 2047; // 0..=2046, never NaN/inf
            let fraction = bits & ((1u64 << 52) - 1);
            f64::from_bits(sign | (biased << 52) | fraction)
        }
    }

    fn randomized<const P: usize, const Q: usize, const R: usize>(seed: u64, rounds: usize) {
        let blades = bivector_blades(P + Q + R);
        let mut rng = XorShift(seed);
        for _ in 0..rounds {
            let mut b = Mv::<P, Q, R>::zero();
            for &index in &blades {
                b.set(index, rng.coeff());
            }
            assert_matches_oracle(&b);
        }
    }

    #[test]
    fn randomized_coefficients_match_bigint() {
        randomized::<2, 1, 0>(0x9E37_79B9_7F4A_7C15, 150);
        randomized::<1, 2, 0>(0xD1B5_4A32_D192_ED03, 150);
        randomized::<3, 1, 0>(0x2545_F491_4F6C_DD1D, 100);
        randomized::<2, 2, 0>(0x94D0_49BB_1331_11EB, 100);
    }

    /// Fill every basis blade (all grades) with a random finite coefficient.
    fn randomized_full<const P: usize, const Q: usize, const R: usize>(seed: u64, rounds: usize) {
        let n = Mv::<P, Q, R>::BASIS_COUNT;
        let mut rng = XorShift(seed);
        for _ in 0..rounds {
            let mut x = Mv::<P, Q, R>::zero();
            for index in 0..n {
                x.set(index, rng.coeff());
            }
            assert_matches_oracle(&x);
        }
    }

    #[test]
    fn randomized_full_multivectors_match_bigint() {
        // General multivectors (not just bivectors): the generalized square
        // must match the full-Cayley-bignum oracle for EVERY output
        // coefficient, including grades 1 and 3.
        randomized_full::<2, 0, 0>(0xA24B_AED4_963E_E407, 120);
        randomized_full::<2, 1, 0>(0x9FB2_1C65_1E9D_2C4F, 120);
        randomized_full::<1, 1, 1>(0xC13F_A703_9B25_81D9, 120);
        randomized_full::<2, 2, 0>(0x0D3E_D4C7_7F1B_66A1, 80);
        randomized_full::<3, 1, 0>(0x71B5_9C8E_44A0_2FD7, 60);
    }

    // ------------------------------------------------------------------
    // (d) extraction pins
    // ------------------------------------------------------------------

    #[test]
    fn extraction_integer_three() {
        // 2e12 + e13 in Cl(2,1,0): exact square -3, angle sqrt(3).
        let b = mv::<2, 1, 0>(&[(2.0, blade(&[1, 2])), (1.0, blade(&[1, 3]))]);
        let square = exact_bivector_square(&b);
        assert!(square.grade4_is_zero());
        let (sign, angle) = square.scalar_sign_and_angle();
        assert_eq!(sign, -1);
        assert_eq!(angle.to_bits(), 3f64.sqrt().to_bits());
    }

    #[test]
    fn extraction_integer_five() {
        // 2e12 + e13 in Cl(1,2,0): exact square +5, angle sqrt(5).
        let b = mv::<1, 2, 0>(&[(2.0, blade(&[1, 2])), (1.0, blade(&[1, 3]))]);
        let square = exact_bivector_square(&b);
        assert!(square.grade4_is_zero());
        let (sign, angle) = square.scalar_sign_and_angle();
        assert_eq!(sign, 1);
        assert_eq!(angle.to_bits(), 5f64.sqrt().to_bits());
    }

    #[test]
    fn extraction_subnormal_single_blade() {
        // 2^-1074 on e12 in Cl(2,0,0): exact square -2^-2148; angle = 2^-1074.
        let c = f64::from_bits(1);
        let b = mv::<2, 0, 0>(&[(c, blade(&[1, 2]))]);
        let square = exact_bivector_square(&b);
        assert!(square.grade4_is_zero());
        let (sign, angle) = square.scalar_sign_and_angle();
        assert_eq!(sign, -1);
        assert_eq!(angle.to_bits(), c.to_bits());
    }

    #[test]
    fn extraction_subnormal_from_raw_accumulator() {
        // The raw accumulator V = 1 is the value 2^-2148; extraction must
        // survive the E = -2148 = 2*(-1074) + 0 composition.
        let mut acc = SignedAccumulator::zero();
        acc.add_term(1, 0, false);
        assert_eq!(acc.sqrt_angle().to_bits(), f64::from_bits(1).to_bits());
        assert_eq!(acc.extracted().unwrap().1, 0);
    }

    #[test]
    fn crafted_dyadic_extraction_within_one_ulp() {
        // Round-4 crafted case: a = 2^22 e12, b = round(sqrt(1023))*2^22 e13,
        // c = 2^27 e23, Cl(1,2,0). Exact square = 962307637273 / 2^40.
        let s = (1u64 << 22) as f64;
        let a = s;
        let b = 1023f64.sqrt() * s;
        let c = (32u64 << 22) as f64;
        assert_eq!(a * a + b * b - c * c, 0.0, "plain f64 false zero");

        let m = mv::<1, 2, 0>(&[
            (a, blade(&[1, 2])),
            (b, blade(&[1, 3])),
            (c, blade(&[2, 3])),
        ]);
        let square = exact_bivector_square(&m);
        assert!(!square.is_zero(), "exact square is nonzero");
        assert!(square.grade4_is_zero());
        assert_eq!(
            accumulator_to_bigint(&square.coeffs[0]),
            BigInt::from(962_307_637_273u64) << 2108,
            "contract exact square 962307637273/2^40 scaled by 2^2148"
        );
        assert_matches_oracle(&m);

        let (sign, angle) = square.scalar_sign_and_angle();
        assert_eq!(sign, 1);
        let reference = f64::from_bits(0x3FED_EFD9_9A5D_8E5B); // 0x1.defd99a5d8e5bp-1
        assert!(
            (angle.to_bits() as i64 - reference.to_bits() as i64).unsigned_abs() <= 1,
            "angle {angle} not within 1 ulp of {reference}"
        );
    }

    // ------------------------------------------------------------------
    // Round-2 remediation: exact-power oracle (bignum x bignum)
    // ------------------------------------------------------------------

    /// A wide-magnitude product must equal the `num-bigint` product of the
    /// two source magnitudes, limb for limb.
    #[test]
    fn mul_magnitude_matches_bigint() {
        let cases: [([u64; WORDS], [u64; WORDS]); 3] = [
            // Identity x a few low words.
            (
                {
                    let mut a = [0u64; WORDS];
                    a[0] = 1;
                    a
                },
                {
                    let mut b = [0u64; WORDS];
                    b[0] = 0xDEAD_BEEF_CAFE_1234;
                    b[1] = 0x1234_5678_9ABC_DEF0;
                    b
                },
            ),
            // A full low half times itself.
            (
                {
                    let mut a = [0u64; WORDS];
                    for (i, slot) in a.iter_mut().enumerate().take(WORDS / 2) {
                        *slot = (i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0x1234_5678;
                    }
                    a
                },
                {
                    let mut b = [0u64; WORDS];
                    for (i, slot) in b.iter_mut().enumerate().take(WORDS / 2) {
                        *slot = (i as u64).wrapping_mul(0xD1B5_4A32_D192_ED03) ^ 0xABCD;
                    }
                    b
                },
            ),
            // Maximal 66-word magnitudes (the proved capacity extreme).
            (
                {
                    let mut a = [u64::MAX; WORDS];
                    a[WORDS - 1] = 0x7FFF_FFFF_FFFF_FFFF;
                    a
                },
                [u64::MAX; WORDS],
            ),
        ];

        for (left, right) in cases {
            let product = mul_magnitude(&left, &right);
            let expected = words_to_biguint(&left) * words_to_biguint(&right);
            assert_eq!(words_to_biguint(&product[..]), expected);
        }
    }

    /// `mul_exact` of two exact powers must reproduce the exact square oracle
    /// coefficientwise (both scaled by `2^2148`).
    #[test]
    fn exact_power_matches_bigint() {
        // Mixed-signature bivector with an integer, a dyadic and an extreme
        // coefficient; the exact square is scalar + grade 4.
        let x = mv::<2, 1, 0>(&[
            (1.5, blade(&[1, 2])),
            (-0.25, blade(&[1, 3])),
            (3.0, blade(&[2, 3])),
        ]);
        let seed = ExactPower::from_multivector(&x).expect("finite coefficients");
        let identity = ExactPower::identity::<2, 1, 0>();

        let first = mul_exact::<2, 1, 0>(&identity, &seed).expect("identity product fits");
        // `identity * y = y`: compare with the direct decomposition.
        for (index, coefficient) in seed.coeffs.iter().enumerate() {
            assert_eq!(
                exact_dyadic_to_scaled_bigint(&first.coeffs[index]),
                exact_dyadic_to_scaled_bigint(coefficient)
            );
        }

        let square = mul_exact::<2, 1, 0>(&seed, &seed).expect("square fits");
        let oracle = oracle_scaled_square(&x);
        for (index, reference) in oracle.iter().enumerate() {
            assert_eq!(
                exact_dyadic_to_scaled_bigint(&square.coeffs[index]),
                *reference,
                "mul_exact coefficient {index} mismatch"
            );
        }
    }

    /// The any-index nilpotent polynomial must be exact for the two canonical
    /// recurring cases: the P2 index-3 example and an index-4 example.
    #[test]
    fn nilpotent_polynomial_matches_closed_form() {
        // P2: x = 1e10 (e1 + e23) in Cl(0,0,3); x^2 = 2e20 e123, x^3 = 0.
        let x = mv::<0, 0, 3>(&[(1e10, blade(&[1])), (1e10, blade(&[2, 3]))]);
        let outcome = exact_nilpotent_exp(&x);
        let NilpotentOutcome::Exp(value) = outcome else {
            panic!("P2 input must be certified nilpotent");
        };
        assert_eq!(value.get(0), 1.0);
        assert_eq!(value.get(blade(&[1])), 1e10);
        assert_eq!(value.get(blade(&[2, 3])), 1e10);
        assert_eq!(value.get(blade(&[1, 2, 3])), 1e20);

        // Index 4: x = e1 + e23 + e45 in Cl(0,0,5).
        // x^2 = 2(e123 + e145 + e2345), x^3 = 6 e12345, x^4 = 0.
        let x = mv::<0, 0, 5>(&[
            (1.0, blade(&[1])),
            (1.0, blade(&[2, 3])),
            (1.0, blade(&[4, 5])),
        ]);
        let NilpotentOutcome::Exp(value) = exact_nilpotent_exp(&x) else {
            panic!("index-4 input must be certified nilpotent");
        };
        assert_eq!(value.get(0), 1.0);
        assert_eq!(value.get(blade(&[1])), 1.0);
        assert_eq!(value.get(blade(&[2, 3])), 1.0);
        assert_eq!(value.get(blade(&[4, 5])), 1.0);
        assert_eq!(value.get(blade(&[1, 2, 3])), 1.0);
        assert_eq!(value.get(blade(&[1, 4, 5])), 1.0);
        assert_eq!(value.get(blade(&[2, 3, 4, 5])), 1.0);
        assert_eq!(value.get(blade(&[1, 2, 3, 4, 5])), 1.0);
    }

    /// D-scheme oracle: the common-denominator exact accumulator must agree
    /// with a direct `num-bigint` evaluation of the exact rational coefficient,
    /// rounded once. Pins the round-3 P1-2 near-cancellation case against a
    /// different computation path.
    #[test]
    fn nilpotent_d_scheme_matches_bigint_rational() {
        // a = nextafter(1e10); x = a·e1 + a·e23 - a²·e123 (index 3).
        let a = f64::from_bits(1e10f64.to_bits() + 1);
        let x = mv::<0, 0, 3>(&[
            (a, blade(&[1])),
            (a, blade(&[2, 3])),
            (-(a * a), blade(&[1, 2, 3])),
        ]);

        // Exact e123 coefficient of `x + x²/2` is `x[7] + a²` (since x² = 2a²·e123).
        // Scale by 2^38 so both dyadic terms are integers, then round once with
        // the same RNE extraction used by the production conversion.
        const SCALE: i32 = 38;
        let (m, e, negative) = decompose(x.get(blade(&[1, 2, 3]))).expect("finite");
        let mut exact = BigInt::from(m) << ((e + SCALE) as usize);
        if negative {
            exact = -exact;
        }
        let (am, ae, _) = decompose(a).expect("finite");
        exact += (BigInt::from(am) * BigInt::from(am)) << ((2 * ae + SCALE) as usize);

        let expected_negative = exact.sign() == Sign::Minus;
        let magnitude = exact.magnitude().clone();
        let (top, b, sticky) = oracle_extract(&magnitude).expect("nonzero rational");
        let (mantissa, carry) = round_mantissa_to_f64(top, sticky);
        let rounded = scale_pow2(mantissa, b as i64 + i64::from(carry) - SCALE as i64);
        let expected = if expected_negative { -rounded } else { rounded };
        assert_eq!(
            expected.to_bits(),
            f64::from_bits(0x40B5_02F9_0000_0004).to_bits(),
            "oracle setup"
        );

        let NilpotentOutcome::Exp(value) = exact_nilpotent_exp(&x) else {
            panic!("crafted input must certify nilpotent");
        };
        assert_eq!(
            value.get(blade(&[1, 2, 3])).to_bits(),
            expected.to_bits(),
            "D-scheme coefficient disagrees with the bignum rational oracle"
        );
        assert_eq!(value.get(0), 1.0);
    }

    /// Round-4 scaler unit tests: the unified symbolic composition must never
    /// form `c * scale_mantissa` in `f64` first.
    #[test]
    fn symbolic_scale_overflow_composition_is_finite() {
        // exp(-0.5) = (0x1.368b2fc6f960ap+0) * 2^-1. The former path formed
        // `f64::MAX * mantissa` (which overflows) before applying the -1 shift.
        let mantissa = f64::from_bits(0x3FF3_68B2_FC6F_960A);
        let value = apply_symbolic_scale(f64::MAX, mantissa, -1);
        assert!(value.is_finite(), "overflow before exponent rescue");
        assert_eq!(value, 1.0903560030489124e308);
    }

    #[test]
    fn symbolic_scale_subnormal_order_rounds_once() {
        // exp(0.4) = (0x1.7de8392fbbfe0p-1) * 2^1. The former path rounded
        // `2^-1074 * mantissa` to the subnormal grid first and then doubled it,
        // yielding from_bits(2); the composition must yield from_bits(1).
        let mantissa = f64::from_bits(0x3FE7_DE83_92FB_BFE0);
        let value = apply_symbolic_scale(f64::from_bits(1), mantissa, 1);
        assert_eq!(value.to_bits(), f64::from_bits(1).to_bits());
    }

    /// Round-5 scaler unit tests: the three-way factor composition and the
    /// exact `1/(2a)` vector finalization.
    #[test]
    fn three_way_symbolic_scale_composes_before_rounding() {
        // A unit factor must reduce to the two-way composition.
        let mantissa = f64::from_bits(0x3FE7_DE83_92FB_BFE0); // exp(0.4)
        assert_eq!(
            apply_symbolic_scale3(f64::MAX, 1.0, mantissa, 1),
            apply_symbolic_scale(f64::MAX, mantissa, 1)
        );
        // `f64::MAX · sinhc(1) · 2^-1` overflows if the factor product is
        // materialized in f64 first; the three-way composition is finite and
        // equals the correctly rounded `e^-1 · sinh(1) · MAX`.
        let sinhc_one = 1.0f64.sinh() / 1.0;
        let scale = symbolic_scale_for_test(-1.0);
        let value = apply_symbolic_scale3(f64::MAX, sinhc_one, scale.0, scale.1);
        assert!(value.is_finite(), "factor product overflowed before rescue");
        assert_eq!(value, f64::from_bits(0x7FDB_AB55_5710_1F8C));
    }

    #[test]
    fn symbolic_scale_over_2a_is_exactly_half_for_c_equals_a() {
        // `c = a` must reproduce `a/(2a) = 0.5` bit-for-bit, including the
        // overflow-adjacent `f64::MAX` and the minimum subnormal, because the
        // divisor never forms `2a` in f64.
        for a in [f64::MAX, 1.0, f64::from_bits(1), 5214.592735436862] {
            let value = apply_symbolic_scale_over_2a(a, a, 1.0, 0);
            assert_eq!(value.to_bits(), 0.5f64.to_bits(), "a = {a:e}");
        }
        // The P1-2b channels must agree when composed against the same symbolic
        // scale: scalar `e^{s+a}/2` and vector `a·e^{s+a}/(2a)`.
        let s = -5926.141552874991_f64;
        let a = 5214.592735436862_f64;
        let (m, e) = symbolic_scale_for_test(s + a);
        let scalar = scale_by_pow2_rne(0.5 * m, e);
        let vector = apply_symbolic_scale_over_2a(a, a, m, e);
        assert_eq!(scalar.to_bits(), vector.to_bits());
        assert_eq!(scalar.to_bits(), 96263207714727);
    }

    // ------------------------------------------------------------------
    // CORE-W04 R1 remediation unit batteries.
    // ------------------------------------------------------------------

    #[test]
    fn exact_ratio_division_is_correctly_rounded() {
        // R1-F3 fixture: w = 1, beta = 0.07081442692123036 in Cl(1,1,0).
        // The old "normalize each significand, then divide" route rounded the
        // exact rational delta = delta_num / N to 0.15242257084684033; the
        // single-division route gives the correctly rounded 0.15242257084684036.
        let beta = ExactPositive::from_f64(0.07081442692123036).expect("positive finite");
        let one = ExactPositive::from_f64(1.0).expect("one");
        let beta_sq = beta.mul(&beta).expect("beta^2");
        let norm_signed = one.sub(&beta_sq).expect("1 - beta^2");
        assert!(!norm_signed.negative);
        let norm = ExactPositive {
            magnitude: norm_signed.magnitude,
            exponent: norm_signed.exponent,
        };
        let wb = one.add(&beta).expect("1 + beta");
        let wb_sq = wb.mul(&wb).expect("(1 + beta)^2");
        let delta_num = wb_sq.sub(&norm).expect("(1 + beta)^2 - N");
        assert!(!delta_num.negative);

        assert_eq!(delta_num.ratio_to_f64(&norm), 0.15242257084684036);
    }

    #[test]
    fn exact_reverse_norm_rejects_signatures_above_max_basis() {
        // R1-F1: the fixed 64-slot table supports n <= 6; larger signatures are
        // reported uncertified rather than constructing an over-wide power that
        // panics in the certificate slice.
        assert_eq!(Mv::<7, 0, 0>::BASIS_COUNT, 128);
        let identity = Mv::<7, 0, 0>::scalar(1.0);
        assert!(matches!(
            exact_reverse_norm(&identity),
            ReverseNorm::Uncertified
        ));
        assert!(exact_reverse_norm_positive(&identity).is_none());
        assert!(exact_reverse_norm_unit(&identity).is_none());
        assert!(matches!(
            exact_reverse_norm_state(&identity),
            ReverseNormState::Uncertified
        ));
        assert!(exact_x2(&identity).is_unsupported());
    }

    /// The test-local copy of the private `symbolic_exp_scale` decomposition
    /// used by the scaler unit tests (kept minimal: the production definition
    /// lives in `lib.rs`).
    fn symbolic_scale_for_test(s: f64) -> (f64, i64) {
        const LOG2_E: f64 = core::f64::consts::LOG2_E;
        const LN2_HI: f64 = f64::from_bits(0x3FE6_2E42_FEE0_0000);
        const LN2_LO: f64 = f64::from_bits(0x3DEA_39EF_3579_3C76);
        let exponent = (s * LOG2_E).round();
        let reduced = (s - exponent * LN2_HI) - exponent * LN2_LO;
        (reduced.exp(), exponent as i64)
    }
}
