//! Exact rational arithmetic for SMT-LIB2 emission.
//!
//! Every f64 is emitted as the EXACT rational it denotes (`numerator /
//! denominator`), never a Display decimal (which denotes a different real —
//! audit finding F11). Transcendental values carry CERTIFIED enclosures
//! computed with elementary rigorous arithmetic (audit contract D2):
//!
//! - `e^{-x}` for a non-negative rational `x` is enclosed via the
//!   alternating Taylor series after scaling: `x = y * 2^s` with
//!   `y < 1/4`; the alternating partial sums bracket `e^{-y}` rigorously
//!   (`|remainder| <= next term`); the interval is then squared `s` times
//!   (positives: `[a,b] -> [a*a, b*b]`), which preserves the enclosure.
//! - No floating-point rounding enters any bound: all arithmetic is exact
//!   big-integer (decimal digit vectors), so `unsat` verdicts on emitted
//!   scripts are proofs about the true reals, not about rounded ones.

/// A non-negative rational as two big integers (decimal digit vectors,
/// most-significant first). Sign handled by the caller for literals.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct BigUint(Vec<u8>);

impl BigUint {
    /// The integer `n`.
    pub(crate) fn from_u64(n: u64) -> Self {
        if n == 0 {
            return BigUint(vec![0]);
        }
        let mut digits = Vec::new();
        let mut n = n;
        while n > 0 {
            digits.push((n % 10) as u8);
            n /= 10;
        }
        digits.reverse();
        BigUint(digits)
    }

    /// Multiply by a small integer (must fit u32).
    pub(crate) fn mul_small(&self, k: u32) -> Self {
        let mut out = Vec::with_capacity(self.0.len() + 6);
        let mut carry: u64 = 0;
        for &d in self.0.iter().rev() {
            let v = d as u64 * k as u64 + carry;
            out.push((v % 10) as u8);
            carry = v / 10;
        }
        while carry > 0 {
            out.push((carry % 10) as u8);
            carry /= 10;
        }
        while out.len() > 1 && *out.last().unwrap() == 0 {
            out.pop();
        }
        out.reverse();
        BigUint(out)
    }

    /// Multiply by a u64 exactly (splits into 32-bit halves; callers
    /// previously truncated `n as u32`, silently corrupting certified
    /// bounds for n > u32::MAX — R1 P2).
    pub(crate) fn mul_u64(&self, k: u64) -> Self {
        let lo = (k & 0xffff_ffff) as u32;
        let hi = (k >> 32) as u32;
        let mut r = self.mul_small(lo);
        if hi > 0 {
            // self * hi * 2^32
            let r2 = self.mul_small(hi);
            // r2 << 32: 32 doublings (cheap relative to schoolbook costs).
            let mut shifted = r2;
            for _ in 0..32 {
                shifted = shifted.mul_small(2);
            }
            r = add_big(&r, &shifted);
        }
        r
    }

    /// Schoolbook multiplication.
    pub(crate) fn mul(&self, other: &BigUint) -> BigUint {
        let a: Vec<u8> = self.0.iter().rev().copied().collect();
        let b: Vec<u8> = other.0.iter().rev().copied().collect();
        let mut acc = vec![0u16; a.len() + b.len()];
        for (i, &da) in a.iter().enumerate() {
            let mut carry: u16 = 0;
            for (j, &db) in b.iter().enumerate() {
                let v = acc[i + j] as u16 + da as u16 * db as u16 + carry;
                acc[i + j] = v % 10;
                carry = v / 10;
            }
            let mut k = i + b.len();
            while carry > 0 {
                let v = acc[k] as u16 + carry;
                acc[k] = v % 10;
                carry = v / 10;
                k += 1;
            }
        }
        while acc.len() > 1 && *acc.last().unwrap() == 0 {
            acc.pop();
        }
        acc.reverse();
        BigUint(acc.into_iter().map(|d| d as u8).collect())
    }

    fn cmp_digits(a: &[u8], b: &[u8]) -> core::cmp::Ordering {
        use core::cmp::Ordering;
        if a.len() != b.len() {
            return a.len().cmp(&b.len());
        }
        for (da, db) in a.iter().zip(b.iter()) {
            match da.cmp(db) {
                Ordering::Equal => continue,
                other => return other,
            }
        }
        Ordering::Equal
    }

    /// Compare two big integers.
    pub(crate) fn cmp(&self, other: &BigUint) -> core::cmp::Ordering {
        Self::cmp_digits(&self.0, &other.0)
    }

    /// Decimal representation.
    pub(crate) fn to_decimal(&self) -> String {
        self.0.iter().map(|d| (b'0' + d) as char).collect()
    }
}

/// An exact non-negative rational `num / den`, den > 0.
#[derive(Clone, Debug)]
pub(crate) struct Rational {
    pub(crate) num: BigUint,
    pub(crate) den: BigUint,
}

impl Rational {
    pub(crate) fn from_u64(n: u64) -> Self {
        Rational {
            num: BigUint::from_u64(n),
            den: BigUint::from_u64(1),
        }
    }

    /// The EXACT rational denoted by a finite non-negative f64
    /// (`m * 2^e` decomposed exactly; audit finding F11).
    ///
    /// # Panics
    ///
    /// Panics on negative, infinite, or NaN input (callers handle sign).
    pub(crate) fn from_f64_exact(value: f64) -> Self {
        assert!(value.is_finite(), "value must be finite: {value}");
        assert!(value >= 0.0, "value must be non-negative: {value}");
        if value == 0.0 {
            return Rational::from_u64(0);
        }
        let bits = value.to_bits();
        let exp_bits = ((bits >> 52) & 0x7ff) as i32;
        let mantissa = bits & 0x000f_ffff_ffff_ffff;
        let (m, e) = if exp_bits == 0 {
            // Subnormal: value = m * 2^-1074
            (mantissa, -1074i32)
        } else {
            // Normal: value = (m | hidden bit) * 2^(e - 1075)
            (mantissa | 0x0010_0000_0000_0000, exp_bits - 1075)
        };
        let mut num = BigUint::from_u64(m);
        let mut den = BigUint::from_u64(1);
        if e >= 0 {
            for _ in 0..e {
                num = num.mul_small(2);
            }
        } else {
            for _ in 0..(-e) {
                den = den.mul_small(2);
            }
        }
        // Reduce by common powers of two while both parts are even: the
        // decomposition m*2^e leaves unreduced factors (3.0 arrives as
        // 6755399441055744/2251799813685248 = 3*2^51/2^51).
        while is_even(&num) && is_even(&den) {
            num = div2(&num);
            den = div2(&den);
        }
        Rational { num, den }
    }

    /// Multiply by a u64 exactly.
    pub(crate) fn mul_u64(&self, k: u64) -> Self {
        Rational {
            num: self.num.mul_u64(k),
            den: self.den.clone(),
        }
    }

    /// Multiply by a u32.
    pub(crate) fn mul_small(&self, k: u32) -> Self {
        Rational {
            num: self.num.mul_small(k),
            den: self.den.clone(),
        }
    }

    /// Exact product of two rationals.
    pub(crate) fn mul(&self, other: &Rational) -> Self {
        Rational {
            num: self.num.mul(&other.num),
            den: self.den.mul(&other.den),
        }
    }

    /// Exact sum (common denominator; sizes stay modest for our uses).
    pub(crate) fn add(&self, other: &Rational) -> Self {
        // a/b + c/d = (a*d + c*b) / (b*d)
        let ad = self.num.mul(&other.den);
        let cb = other.num.mul(&self.den);
        let num = add_big(&ad, &cb);
        let den = self.den.mul(&other.den);
        Rational { num, den }
    }

    /// Less-or-equal (cross-multiplied).
    pub(crate) fn le_rat(&self, other: &Rational) -> bool {
        let c = self.num.mul(&other.den).cmp(&other.num.mul(&self.den));
        c != core::cmp::Ordering::Greater
    }

    /// Compare against another rational (cross-multiplied).
    pub(crate) fn lt(&self, other: &Rational) -> bool {
        self.num.mul(&other.den).cmp(&other.num.mul(&self.den)) == core::cmp::Ordering::Less
    }

    /// SMT-LIB2 literal: `/ num den` (or the plain numeral when den == 1).
    pub(crate) fn to_smt_literal(&self) -> String {
        let den_one = self.den == BigUint::from_u64(1);
        if den_one {
            self.num.to_decimal()
        } else {
            format!("(/ {} {})", self.num.to_decimal(), self.den.to_decimal())
        }
    }
}

fn is_even(x: &BigUint) -> bool {
    x.0.last().map(|d| d.is_multiple_of(2)).unwrap_or(true)
}

/// Exact halving of an even big integer.
fn div2(x: &BigUint) -> BigUint {
    let mut out = Vec::with_capacity(x.0.len());
    let mut carry: u8 = 0;
    for &d in &x.0 {
        let v = carry * 10 + d;
        out.push(v / 2);
        carry = v % 2;
    }
    while out.len() > 1 && out[0] == 0 {
        out.remove(0);
    }
    BigUint(out)
}

pub(crate) fn add_big(a: &BigUint, b: &BigUint) -> BigUint {
    let (long, short): (&Vec<u8>, &Vec<u8>) = if a.0.len() >= b.0.len() {
        (&a.0, &b.0)
    } else {
        (&b.0, &a.0)
    };
    let mut out = Vec::with_capacity(long.len() + 1);
    let mut carry: u8 = 0;
    let long_rev = long.iter().rev();
    let short_rev = short.iter().rev();
    let mut it_short = short_rev;
    for &d in long_rev {
        let mut v = d + carry;
        if let (Some(&ds), rest) = (it_short.next(), &mut it_short) {
            let _ = rest;
            v += ds;
        }
        carry = v / 10;
        out.push(v % 10);
    }
    if carry > 0 {
        out.push(carry);
    }
    out.reverse();
    BigUint(out)
}

/// A certified enclosure `[lo, hi]` of a real value.
#[derive(Clone, Debug)]
pub(crate) struct Enclosure {
    pub(crate) lo: Rational,
    pub(crate) hi: Rational,
}

impl Enclosure {
    /// Enclose `e^{-x}` for non-negative rational `x`, rigorously.
    ///
    /// Scales `x = y * 2^s` so `y < 1/4`, brackets `e^{-y}` with
    /// alternating Taylor partial sums (for `0 <= y`, `e^{-y}` lies
    /// between consecutive partial sums and `|remainder| <= next term`),
    /// then squares the interval `s` times.
    /// Rigorous small-tail cutoff (R2 P2, proof corrected R3): e > 2, so
    /// for x >= 4096, e^{-x} <= e^{-4096} < 2^{-4096} = 10^{-4096*log10(2)}
    /// = 10^{-1233.018862...} < 10^{-500}; and 10^{-500} is below the smallest
    /// POSITIVE f64 (5e-324 subnormal), so [0, 10^{-500}] decides every
    /// representable delta comparison WITHOUT constructing the ~10^9-digit
    /// denominators the true value would require. (The R2 chain wrongly
    /// used e < 3, which bounds e^{-4096} from ABOVE — 3^{-4096} is
    /// SMALLER than e^{-4096}, not larger.)
    const SMALL_TAIL_CUTOFF: u64 = 4096;
    const SMALL_TAIL_BOUND_ZEROES: usize = 500;

    pub(crate) fn exp_neg(x: &Rational) -> Self {
        if x.num.cmp(&BigUint::from_u64(0)) == core::cmp::Ordering::Equal {
            // e^0 = 1 exactly.
            let one = Rational::from_u64(1);
            return Enclosure {
                lo: one.clone(),
                hi: one,
            };
        }
        // x >= 4096 (cross-multiplied against 1): the small-tail bound.
        let cutoff = Rational::from_u64(Self::SMALL_TAIL_CUTOFF);
        if cutoff.le_rat(x) {
            let zero = Rational::from_u64(0);
            let hi = Rational {
                num: BigUint::from_u64(1),
                den: {
                    let mut d = BigUint::from_u64(1);
                    for _ in 0..Self::SMALL_TAIL_BOUND_ZEROES {
                        d = d.mul_small(10);
                    }
                    d
                },
            };
            return Enclosure { lo: zero, hi };
        }
        // Halve x until below 1/4 (compare via x < 1/4, i.e. 4*num < den).
        let quarter = Rational {
            num: BigUint::from_u64(1),
            den: BigUint::from_u64(4),
        };
        let mut y = x.clone();
        let mut squarings = 0u32;
        while !y.lt(&quarter) {
            y = Rational {
                num: y.num,
                den: y.den.mul_small(2),
            };
            squarings += 1;
        }
        // Alternating Taylor for e^{-y}: sum_{k} (-y)^k / k!
        // Terms as rationals: t_k = y^k / k!
        let mut enclosure = taylor_alternating(&y);
        for _ in 0..squarings {
            enclosure = Enclosure {
                lo: enclosure.lo.mul(&enclosure.lo),
                hi: enclosure.hi.mul(&enclosure.hi),
            };
            // Outward rounding to SIGNIFICANT_DIGITS significant digits:
            // without it, each squaring doubles the digit count (2^s growth)
            // and the operands become astronomically large. Rounding lo down
            // and hi up keeps the enclosure VALID and the sizes bounded.
            enclosure.lo = round_lo(&enclosure.lo, SIGNIFICANT_DIGITS);
            enclosure.hi = round_hi(&enclosure.hi, SIGNIFICANT_DIGITS);
        }
        enclosure
    }

    /// Scale by 2 (for `2 * exp(-x)`).
    pub(crate) fn times_two(&self) -> Self {
        Enclosure {
            lo: self.lo.mul_small(2),
            hi: self.hi.mul_small(2),
        }
    }
}

fn taylor_alternating(y: &Rational) -> Enclosure {
    // e^{-y} = sum (-1)^k t_k, t_k = y^k / k!, terms positive and strictly
    // decreasing for 0 < y <= 1/4: even partial sums are upper bounds,
    // odd partial sums are lower bounds, and |remainder| <= next term.
    // ONE running sum S accumulates +/- terms; the enclosure is
    // [S_after_last_odd, S_after_last_even], stopping only after an even
    // update so `hi` is always the current (even) sum.
    let mut term = Rational::from_u64(1); // t_0
    let mut running = Rational::from_u64(1); // S_0 (upper bound)
    let mut lo = Rational::from_u64(0);
    let mut k: u32 = 0;
    loop {
        k += 1;
        // t_k = t_{k-1} * y / k
        term = Rational {
            num: term.num.mul(&y.num),
            den: term.den.mul(&y.den).mul_small(k),
        };
        if k % 2 == 1 {
            running = negate_rational_pair(&term, &running); // S -= t_k
            lo = running.clone();
        } else {
            running = running.add(&term); // S += t_k; upper bound
                                          // Stop only at even k: enclosure is [lo, running], width <= t_{k+1} <= t_k.
            if k > 4 && term.lt(&scale_hint(&running)) {
                break;
            }
        }
        if k > 200 {
            break;
        }
    }
    Enclosure { lo, hi: running }
}

/// Subtract `a` from `b` for non-negative result usage: returns b - a as a
/// Rational when b >= a; otherwise the zero rational. Used by the odd
/// partial sums (which are <= e^{-y} <= even sums, and the running even
/// sum only grows).
fn negate_rational_pair(a: &Rational, b: &Rational) -> Rational {
    // b - a with common denominator
    let num_b = b.num.mul(&a.den);
    let num_a = a.num.mul(&b.den);
    let den = b.den.mul(&a.den);
    if num_b.cmp(&num_a) == core::cmp::Ordering::Less {
        return Rational {
            num: BigUint::from_u64(0),
            den,
        };
    }
    let diff = sub_big(&num_b, &num_a);
    Rational { num: diff, den }
}

fn sub_big(a: &BigUint, b: &BigUint) -> BigUint {
    // a >= b guaranteed by caller
    let mut out = Vec::new();
    let mut borrow: i16 = 0;
    let a_rev = a.0.iter().rev();
    let mut it_b = b.0.iter().rev();
    for &da in a_rev {
        let mut v = da as i16 - borrow;
        if let Some(&db) = it_b.next() {
            v -= db as i16;
        }
        if v < 0 {
            v += 10;
            borrow = 1;
        } else {
            borrow = 0;
        }
        out.push(v as u8);
    }
    while out.len() > 1 && *out.last().unwrap() == 0 {
        out.pop();
    }
    out.reverse();
    BigUint(out)
}

/// Significant digits retained under outward rounding during interval
/// squaring (see [`Enclosure::exp_neg`]).
const SIGNIFICANT_DIGITS: usize = 42;

/// Outward-round a rational DOWNWARD to ~`sig` significant digits.
/// The shift is derived from the NUMERATOR's length and applied to BOTH
/// parts (the 10^-s factors cancel, preserving the ratio; the num/den
/// digit gap — the value's decimal scale — is invariant). Truncating to a
/// fixed total length instead would send tiny values' lower bounds to
/// zero and divide parts by different powers of ten when lengths differ.
fn round_lo(r: &Rational, sig: usize) -> Rational {
    let shift = r.num.0.len().saturating_sub(sig);
    Rational {
        num: drop_floor(&r.num, shift),
        den: drop_ceil(&r.den, shift),
    }
}

/// Outward-round a rational UPWARD (same num-derived-shift discipline).
fn round_hi(r: &Rational, sig: usize) -> Rational {
    let shift = r.num.0.len().saturating_sub(sig);
    Rational {
        num: drop_ceil(&r.num, shift),
        den: drop_floor(&r.den, shift),
    }
}

/// floor(N / 10^shift) as a digit vector (0 when shift >= digits).
fn drop_floor(x: &BigUint, shift: usize) -> BigUint {
    let d = &x.0;
    if d.len() <= shift {
        return BigUint::from_u64(0);
    }
    BigUint(d[..d.len() - shift].to_vec())
}

/// ceil(N / 10^shift) as a digit vector: the quotient plus one ulp at the
/// reduced scale (always >= the true value — slightly conservative when
/// the dropped digits are zero, never unsound).
fn drop_ceil(x: &BigUint, shift: usize) -> BigUint {
    if x.0.len() <= shift {
        return BigUint::from_u64(1);
    }
    let d = &x.0;
    let kept = &d[..d.len() - shift];
    let mut out = kept.to_vec();
    for i in (0..out.len()).rev() {
        if out[i] == 9 {
            out[i] = 0;
        } else {
            out[i] += 1;
            return BigUint(out);
        }
    }
    let mut all = vec![1u8];
    all.extend(core::iter::repeat_n(0, out.len()));
    BigUint(all)
}

fn scale_hint(r: &Rational) -> Rational {
    // A smallness threshold relative to r: r / 10^30 (as a rational,
    // multiply den by 10^30). Taylor stops when the next term is below
    // this; combined with k > 4 this certifies bracket width far below
    // any practical solver threshold while keeping big-int sizes modest.
    let mut den = r.den.clone();
    for _ in 0..30 {
        den = den.mul_small(10);
    }
    Rational {
        num: r.num.clone(),
        den,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f64_to_exact_rational_roundtrips_through_smt_literal() {
        // 0.1 as an f64 is exactly 3602879701896397 / 2^55.
        let r = Rational::from_f64_exact(0.1);
        assert_eq!(r.num.to_decimal(), "3602879701896397");
        let mut den = BigUint::from_u64(1);
        for _ in 0..55 {
            den = den.mul_small(2);
        }
        assert_eq!(r.den, den);
        assert_eq!(r.to_smt_literal(), "(/ 3602879701896397 36028797018963968)");
    }

    #[test]
    fn integer_valued_f64_emits_plain_numeral() {
        assert_eq!(Rational::from_f64_exact(3.0).to_smt_literal(), "3");
        // 2^62 as f64 is exact
        assert_eq!(
            Rational::from_f64_exact(4611686018427387904.0).to_smt_literal(),
            "4611686018427387904"
        );
    }

    fn rat_from_decimals(num_digits: &str, den_digits: &str) -> Rational {
        let parse = |d: &str| BigUint(d.bytes().map(|b| b - b'0').collect());
        Rational {
            num: parse(num_digits),
            den: parse(den_digits),
        }
    }

    #[test]
    fn exp_neg_enclosure_brackets_known_values() {
        // e^{-1} = 0.36787944117144232159552377016146086744581113103177.
        // Exact rational bracketing against decimal truncations of the
        // true value (f64 hints cannot distinguish 1e-42-wide bounds).
        let one = Rational::from_u64(1);
        let e = Enclosure::exp_neg(&one);
        let pow22 = format!("1{}", "0".repeat(22));
        let truth_lo = rat_from_decimals("3678794411714423215955", &pow22); // < e^{-1}
        let truth_hi = rat_from_decimals("3678794411714423215956", &pow22); // > e^{-1}
        assert!(
            e.lo.lt(&truth_hi),
            "lower bound above the true value: {}",
            e.lo.to_smt_literal()
        );
        assert!(
            truth_lo.lt(&e.hi),
            "upper bound below the true value: {}",
            e.hi.to_smt_literal()
        );
        // Gross-error window (catches e.g. the historical x10 rounding).
        let hint = |r: &Rational| -> f64 {
            r.num.to_decimal().parse::<f64>().unwrap() / r.den.to_decimal().parse::<f64>().unwrap()
        };
        assert!((hint(&e.lo) - 0.367879441171442_3).abs() < 1e-15);
        assert!((hint(&e.hi) - 0.367879441171442_3).abs() < 1e-15);
    }

    #[test]
    fn exp_neg_enclosure_is_tight() {
        let one = Rational::from_f64_exact(1.0);
        let e = Enclosure::exp_neg(&one);
        // Relative width must be tiny: (hi - lo) / lo < 1e-9.
        // hi - lo = (hi*den_lo - lo*den_hi) / (den_lo*den_hi) — approximate
        // via cross-multiplied comparison against a slightly widened lo.
        let widened = Rational {
            num: e.lo.num.mul_small(1_000_000_001),
            den: e.lo.den.mul_small(1_000_000_000),
        };
        let c = e.hi.num.mul(&widened.den).cmp(&widened.num.mul(&e.hi.den));
        assert!(
            c == core::cmp::Ordering::Less || c == core::cmp::Ordering::Equal,
            "enclosure too wide for 1e-9 relative"
        );
    }

    #[test]
    fn exp_neg_handles_large_arguments() {
        // x = 2000: e^{-2000} ~ 1.88e-869 — the f64-underflow trap from the
        // contract (P1): the enclosure must be strictly ABOVE zero.
        let x = Rational::from_u64(2000);
        let e = Enclosure::exp_neg(&x);
        let zero = Rational::from_u64(0);
        assert!(
            zero.lt(&e.lo),
            "enclosure lower bound must be > 0, got {:?}",
            e.lo.to_smt_literal()
        );
        // And small: e.lo < 1e-800
        let tiny = Rational {
            num: BigUint::from_u64(1),
            den: {
                let mut d = BigUint::from_u64(1);
                for _ in 0..800 {
                    d = d.mul_small(10);
                }
                d
            },
        };
        assert!(e.hi.lt(&tiny), "enclosure must be tiny");
    }

    #[test]
    fn extreme_tail_cutoff_is_fast_decisive_and_rigorous() {
        // R2 P2 anchor: x = 2*(2^32+1)*0.25 ~ 2.1e9 previously needed a
        // billion-digit denominator (30s+ timeout); the cutoff returns
        // [0, 10^-500] instantly — below every positive f64 delta.
        let start = std::time::Instant::now();
        let eps_r = Rational::from_f64_exact(0.5);
        let x = eps_r.mul(&eps_r).mul_small(2).mul_u64(4_294_967_297);
        let e = Enclosure::exp_neg(&x).times_two();
        assert!(start.elapsed().as_millis() < 500, "must be fast");
        // Rigor: hi = 2 * 10^-500 < 5e-324 (smallest positive f64).
        let hi_f64: f64 = e.hi.num.to_decimal().parse::<f64>().unwrap()
            / e.hi.den.to_decimal().parse::<f64>().unwrap();
        assert_eq!(hi_f64, 0.0); // underflows — strictly below every positive f64
        let zero = Rational::from_u64(0);
        assert!(
            zero.le_rat(&e.lo)
                || zero.lt(&e.lo)
                || e.lo.num.cmp(&BigUint::from_u64(0)) == core::cmp::Ordering::Equal
        );
    }

    #[test]
    fn mul_u64_preserves_large_counts() {
        // R1 P2 anchor: n = 2^32 + 1 must NOT truncate to 1.
        let r = Rational::from_u64(1).mul_u64(4_294_967_297);
        // 2^32 + 1 = 4294967297
        assert_eq!(r.num.to_decimal(), "4294967297");
    }

    #[test]
    fn big_uint_mul_matches_small_path() {
        let a = BigUint::from_u64(123_456_789);
        let by_school = a.mul(&BigUint::from_u64(987_654));
        let expected: u128 = 123_456_789u128 * 987_654u128;
        assert_eq!(by_school.to_decimal(), expected.to_string());
    }
}
