//! Rotor operations for rotations and reflections

use crate::error::RotorError;
use crate::exact_square::{
    divide_f64_by_positive, exact_product_eq, exact_reverse_norm, exact_reverse_norm_positive,
    exact_reverse_norm_state, exact_reverse_norm_unit, exact_x2, ExactPositive, ReverseNorm,
    ReverseNormState,
};
use crate::has_foreign_grade;
use crate::{Bivector, Multivector, Vector};
use alloc::vec::Vec;
use core::cmp::Ordering;

/// Rotor in a Clifford algebra (even-grade multivector with unit norm)
pub struct Rotor<const P: usize, const Q: usize, const R: usize> {
    multivector: Multivector<P, Q, R>,
}

impl<const P: usize, const Q: usize, const R: usize> Rotor<P, Q, R> {
    /// Create a rotor from a bivector using exponential map
    ///
    /// For a bivector B representing a plane and angle θ,
    /// the rotor R = exp(-B*θ/2) performs a right-handed rotation by angle θ in that plane.
    ///
    /// The negative sign ensures the rotation follows the right-hand rule convention:
    /// when the thumb points in the direction of the bivector orientation,
    /// the fingers curl in the direction of positive rotation.
    ///
    /// Returns the exponential's error if the half-angle bivector has no
    /// representable exponential (see [`Multivector::exp`]), or
    /// [`RotorError::NotNormalizable`] when the exponential has no positive
    /// square to normalize by, or [`RotorError::Degenerate`] when the stored
    /// reverse norm is not a positive scalar.
    pub fn from_bivector(bivector: &Bivector<P, Q, R>, angle: f64) -> Result<Self, RotorError> {
        // Negative for right-handed rotation.
        let half_angle_bivector = bivector.as_multivector() * (-angle / 2.0);
        let rotor = half_angle_bivector.exp()?;

        Self::normalize_and_certify(rotor)
    }

    /// Certify the stored positive scalar reverse norm `N = R·R† > 0` and
    /// return the element unchanged.
    ///
    /// Contract §1.4b output gate. The exponential-route powers preserve the
    /// element scale the logarithm carries, so they certify the un-normalized
    /// exponential output rather than rescaling it to unit (CORE-W04 R1-F4).
    fn certify_exact_scalar_n(multivector: Multivector<P, Q, R>) -> Result<Self, RotorError> {
        match exact_reverse_norm(&multivector) {
            ReverseNorm::Positive => Ok(Self { multivector }),
            _ => Err(RotorError::Degenerate),
        }
    }

    /// Normalize a checked exponential output and certify the stored positive
    /// scalar reverse norm (contract §1.4b output gate).
    ///
    /// The former silent identity fallback is deleted: a non-normalizable
    /// exponential is [`RotorError::NotNormalizable`], and an output whose
    /// stored `N = R·R†` is non-positive or non-scalar is
    /// [`RotorError::Degenerate`] (representability limit).
    fn normalize_and_certify(multivector: Multivector<P, Q, R>) -> Result<Self, RotorError> {
        // Certify the stored reverse norm before any normalization: the
        // representability limit (e.g. the exact `cosh(32·ln2)` rounding to a
        // stored `N = 0` at magnitude) must surface as
        // [`RotorError::Degenerate`], never as a silent identity fallback or a
        // bare normalization failure.
        let certified = Self::certify_exact_scalar_n(multivector)?;
        let normalized = certified
            .multivector
            .normalize_versor()
            .ok_or(RotorError::NotNormalizable)?;
        Self::certify_exact_scalar_n(normalized)
    }

    /// Create a rotor from a raw multivector bivector
    ///
    /// Similar to `from_bivector` but accepts a raw `Multivector` representing a bivector.
    /// Uses the same right-handed rotation convention.
    ///
    /// Returns the exponential's error if the half-angle bivector has no
    /// representable exponential (see [`Multivector::exp`]),
    /// [`RotorError::NotABivector`] when `bivector` has a nonzero foreign-grade
    /// coefficient, or the normalization/degeneracy errors of
    /// [`Self::from_bivector`].
    pub fn from_multivector_bivector(
        bivector: &Multivector<P, Q, R>,
        angle: f64,
    ) -> Result<Self, RotorError> {
        // Exact grade-2 validation (contract §1.3): any nonzero foreign-grade
        // coefficient rejects. `±0.0` is zero; NaN/inf in a foreign grade is
        // nonzero and rejects.
        if has_foreign_grade(bivector, 2) {
            return Err(RotorError::NotABivector);
        }

        // Negative for right-handed rotation.
        let half_angle_bivector = bivector * (-angle / 2.0);
        let rotor = half_angle_bivector.exp()?;

        Self::normalize_and_certify(rotor)
    }

    /// Apply rotor to transform a multivector: `R·v·R† / N`.
    ///
    /// The stored reverse norm `N = R·R†` must certify scalar and positive;
    /// otherwise [`RotorError::Degenerate`]. Exactly-normalized rotors behave
    /// as the bare sandwich within rounding. The division is performed per
    /// coefficient against the **exact** `N` dyadic, so each output rounds
    /// once (CORE-W04 R1-F2); no pre-rounded `f64` denominator is used.
    pub fn apply(&self, v: &Multivector<P, Q, R>) -> Result<Multivector<P, Q, R>, RotorError> {
        let n = exact_reverse_norm_positive(&self.multivector).ok_or(RotorError::Degenerate)?;
        let r_dagger = self.multivector.reverse();
        let product = self
            .multivector
            .geometric_product(v)
            .geometric_product(&r_dagger);
        let mut result = Multivector::zero();
        for index in 0..Multivector::<P, Q, R>::BASIS_COUNT {
            result.set(index, divide_f64_by_positive(product.get(index), &n));
        }
        Ok(result)
    }

    /// Get the underlying multivector
    pub fn as_multivector(&self) -> &Multivector<P, Q, R> {
        &self.multivector
    }

    /// Compose two rotors (raw geometric product).
    ///
    /// This is the **documented domain-exit point** (contract §1.5): the raw
    /// product is infallible and does not normalize, so chained compositions
    /// drift from unit and may store `N <= 0`; every checked operation detects
    /// that.
    pub fn compose(&self, other: &Self) -> Self {
        Self {
            multivector: self.multivector.geometric_product(&other.multivector),
        }
    }

    /// Get the inverse rotor `R† / N`, where `N = R·R†` is the exact stored
    /// reverse norm (contract §1.4b).
    ///
    /// The stored element's inverse, not a normalized-value inverse: the
    /// reverse-norm division corrects the stored scalar scale. Works for
    /// negative-sheet elements (their `N > 0`). `N <= 0` or non-scalar `N` is
    /// [`RotorError::Degenerate`].
    ///
    /// Each reversed coefficient is divided by the **exact** `N` dyadic with a
    /// single rounding at the destination precision (CORE-W04 R1-F2); the
    /// denominator is never pre-rounded to `f64`.
    pub fn inverse(&self) -> Result<Self, RotorError> {
        let n = exact_reverse_norm_positive(&self.multivector).ok_or(RotorError::Degenerate)?;
        let reversed = self.multivector.reverse();
        let mut result = Multivector::zero();
        for index in 0..Multivector::<P, Q, R>::BASIS_COUNT {
            result.set(index, divide_f64_by_positive(reversed.get(index), &n));
        }
        Ok(Self {
            multivector: result,
        })
    }

    /// Create identity rotor
    pub fn identity() -> Self {
        Self {
            multivector: Multivector::scalar(1.0),
        }
    }

    /// Coefficient magnitude (versor metric policy deferred to W03).
    pub fn magnitude(&self) -> f64 {
        self.multivector.coefficient_norm()
    }

    /// Get scalar part of the rotor
    pub fn scalar_part(&self) -> f64 {
        self.multivector.scalar_part()
    }

    /// Get bivector part of the rotor
    pub fn bivector_part(&self) -> Bivector<P, Q, R> {
        self.multivector.bivector_type()
    }

    /// Get coefficients as slice
    pub fn as_slice(&self) -> &[f64] {
        self.multivector.as_slice()
    }

    /// Apply rotor to a Vector.
    ///
    /// The sandwich is the same certified-`N` operation as [`Self::apply`];
    /// the result is projected to its grade-0+1 vector part (documented
    /// defense-in-depth against stored-scale drift, contract §1.4).
    pub fn apply_to_vector(&self, v: &Vector<P, Q, R>) -> Result<Vector<P, Q, R>, RotorError> {
        let result = self.apply(v.as_multivector())?;
        Ok(Vector::from_grade_projection(&result))
    }

    /// Geometric product with another rotor
    pub fn geometric_product(&self, other: &Self) -> Self {
        self.compose(other)
    }

    /// Create rotor from vectors (typed version).
    ///
    /// Routes through [`Self::from_vectors_mv`] (contract §3a). `Option` is
    /// replaced by named errors: pure-vector validation →
    /// [`RotorError::NotAVector`], null inputs → [`RotorError::NullVector`],
    /// opposite causal type → [`RotorError::CausalTypeMismatch`], degenerate
    /// connecting versors → [`RotorError::NullCandidate`] /
    /// [`RotorError::SheetChange`].
    pub fn from_vectors(a: &Vector<P, Q, R>, b: &Vector<P, Q, R>) -> Result<Self, RotorError> {
        Self::from_vectors_mv(a.as_multivector(), b.as_multivector())
    }

    /// Create a rotor mapping `a` to `b` (contract §3a).
    ///
    /// Precedence: exact pure-vector validation (`NotAVector`) → exact scalar
    /// square `σ(x) = x·x` (`NullVector`, `CausalTypeMismatch`) → direction
    /// normalization → exact 2×2-minor proportionality pre-test on the original
    /// coefficients (antipodal handling) → candidate trichotomy
    /// `C = 1 + s·b̂â` normalized by its certified `CC†`.
    pub fn from_vectors_mv(
        a: &Multivector<P, Q, R>,
        b: &Multivector<P, Q, R>,
    ) -> Result<Self, RotorError> {
        // §3a precedence: pure-vector validation before any square processing.
        if has_foreign_grade(a, 1) || has_foreign_grade(b, 1) {
            return Err(RotorError::NotAVector);
        }

        // Certified `σ(x) = x·x` on the original inputs (exact dyadic square).
        let (sign_a, beta_a) = Self::certified_vector_square(a)?;
        let (sign_b, beta_b) = Self::certified_vector_square(b)?;
        if sign_a == 0 || sign_b == 0 {
            return Err(RotorError::NullVector);
        }
        if sign_a != sign_b {
            return Err(RotorError::CausalTypeMismatch);
        }
        let s = sign_a;

        let a_hat = Self::normalize_direction(a, beta_a)?;
        let b_hat = Self::normalize_direction(b, beta_b)?;

        // §3a.4 direction pre-test on the certified ORIGINAL coefficients:
        // exact proportionality is signature-free, so it is decided before the
        // candidate (which is metric-sensitive).
        if Self::vectors_proportional(a, b) && Self::proportionality_sign(a, b) < 0 {
            return Self::resolve_antipodal(&a_hat, &b_hat);
        }

        // §3a.3/§3a.5 candidate trichotomy `C = 1 + s·b̂â`.
        let candidate = Self::direction_candidate(&a_hat, &b_hat, s);
        if candidate.is_zero() {
            return Self::resolve_antipodal(&a_hat, &b_hat);
        }
        match exact_reverse_norm_state(&candidate) {
            ReverseNormState::Positive => Self::normalize_and_certify(candidate),
            ReverseNormState::Zero => Err(RotorError::NullCandidate),
            ReverseNormState::Negative => Err(RotorError::SheetChange),
            ReverseNormState::NonScalar | ReverseNormState::Uncertified => {
                Err(RotorError::NotNormalizable)
            }
        }
    }

    /// Certified exact scalar `σ(x) = x·x` of a pure grade-1 input.
    ///
    /// Returns `(sign, sqrt(|σ|))`; any uncertifiable square (unsupported
    /// signature, non-finite coefficient, or non-scalar result) is
    /// [`RotorError::NotNormalizable`].
    fn certified_vector_square(v: &Multivector<P, Q, R>) -> Result<(i32, f64), RotorError> {
        let square = exact_x2(v);
        if square.is_unsupported() || square.has_non_finite_source() || !square.is_exactly_scalar()
        {
            return Err(RotorError::NotNormalizable);
        }
        Ok(square.scalar_sign_and_angle())
    }

    /// Sign-aware direction normalization `v / sqrt(|σ(v)|)`, finite-guarded.
    fn normalize_direction(
        v: &Multivector<P, Q, R>,
        beta: f64,
    ) -> Result<Multivector<P, Q, R>, RotorError> {
        if !beta.is_finite() || beta <= 0.0 {
            return Err(RotorError::NotNormalizable);
        }
        let normalized = v / beta;
        if normalized
            .as_slice()
            .iter()
            .any(|coefficient| !coefficient.is_finite())
        {
            return Err(RotorError::NotNormalizable);
        }
        Ok(normalized)
    }

    /// Exact proportionality of the original pure-vector coefficient vectors
    /// via all `2×2` minors `aᵢbⱼ − aⱼbᵢ = 0` (contract §3a.4).
    fn vectors_proportional(a: &Multivector<P, Q, R>, b: &Multivector<P, Q, R>) -> bool {
        for i in 0..Multivector::<P, Q, R>::DIM {
            for j in (i + 1)..Multivector::<P, Q, R>::DIM {
                let ai = a.get(1 << i);
                let aj = a.get(1 << j);
                let bi = b.get(1 << i);
                let bj = b.get(1 << j);
                if !exact_product_eq(ai, bj, aj, bi) {
                    return false;
                }
            }
        }
        true
    }

    /// Sign of the proportionality factor from `sign(aᵢ·bᵢ)` at the first
    /// index with `bᵢ ≠ 0` (contract §3a.4).
    fn proportionality_sign(a: &Multivector<P, Q, R>, b: &Multivector<P, Q, R>) -> i32 {
        for k in 0..Multivector::<P, Q, R>::DIM {
            let ak = a.get(1 << k);
            let bk = b.get(1 << k);
            if bk != 0.0 {
                if ak == 0.0 {
                    continue;
                }
                return if (ak > 0.0) == (bk > 0.0) { 1 } else { -1 };
            }
        }
        0
    }

    /// Candidate `C = 1 + s·b̂â` (contract §3a.3).
    fn direction_candidate(
        a_hat: &Multivector<P, Q, R>,
        b_hat: &Multivector<P, Q, R>,
        s: i32,
    ) -> Multivector<P, Q, R> {
        let ba = b_hat.geometric_product(a_hat);
        Multivector::scalar(1.0) + ba * (s as f64)
    }

    /// Antipodal resolution: definite Euclidean 2D only, i.e. `(P, Q, R)`
    /// equal to `(2, 0, 0)` or `(0, 2, 0)`, via the unique plane `±e12`.
    ///
    /// Every indefinite or higher signature returns [`RotorError::Antipodal`]:
    /// the plane element does not implement `â -> -â` there. In Cl(1,1,0) the
    /// plane `e12` has `e12² = +1`, so `e12·e12† = -1 < 0` (negative reverse
    /// norm) and no checked rotor maps `â` to `-â`; note in particular
    /// `e12·e1·e12 = -e1`, so `e12` is **not** the identity action (CORE-W04
    /// R1-F6).
    fn resolve_antipodal(
        a_hat: &Multivector<P, Q, R>,
        b_hat: &Multivector<P, Q, R>,
    ) -> Result<Self, RotorError> {
        let definite_2d = (P == 2 && Q == 0) || (P == 0 && Q == 2);
        if !definite_2d || R != 0 {
            return Err(RotorError::Antipodal);
        }
        let mut plane = Multivector::zero();
        plane.set(0b11, 1.0); // e12
        let rotor = Self::normalize_and_certify(plane)?;
        let acted = rotor.apply(a_hat)?;
        if acted.approx_eq(b_hat, 1e-12) {
            Ok(rotor)
        } else {
            Err(RotorError::Antipodal)
        }
    }

    /// Spherical linear interpolation between rotors (contract §2e).
    ///
    /// `t = 0` returns `self` exactly and `t = 1` returns the sign-selected
    /// representative `B*` exactly (bit-exact direct returns). The sign rule is
    /// the **scalar coefficient dot** `⟨A,B⟩_dot = Σᵢ AᵢBᵢ` over all
    /// coefficients: strictly negative selects `B* = −B`; a tie at exactly
    /// zero keeps `B`. Interior `t` evaluates `A·(A⁻¹B*)^t` through the
    /// certified logarithm/power route, with the output-`N` gate applied.
    ///
    /// The former hard-coded 8-coefficient loop is deleted; the general
    /// coefficient vector is used throughout.
    pub fn slerp(&self, other: &Self, t: f64) -> Result<Self, RotorError> {
        // §2e endpoints are direct returns.
        if t == 0.0 {
            return Ok(Self {
                multivector: self.multivector.clone(),
            });
        }

        // Sign rule: scalar coefficient dot over ALL coefficients (the
        // quaternion convention, not `Multivector::inner_product`).
        let dot: f64 = self
            .as_slice()
            .iter()
            .zip(other.as_slice().iter())
            .map(|(&a, &b)| a * b)
            .sum();
        let b_star = if dot < 0.0 {
            Self {
                multivector: -other.multivector.clone(),
            }
        } else {
            Self {
                multivector: other.multivector.clone(),
            }
        };

        if t == 1.0 {
            return Ok(b_star);
        }

        // `A = B` (selected representative equal) → `A` exactly.
        if self.multivector == b_star.multivector {
            return Ok(Self {
                multivector: self.multivector.clone(),
            });
        }

        // Interior `t`: `A·(A⁻¹B*)^t` through the certified log/power route.
        let a_inverse = self.inverse()?;
        let relative = a_inverse.compose(&b_star);
        let powered = relative.power(t)?;
        let result = self.multivector.geometric_product(&powered.multivector);
        Self::normalize_and_certify(result)
    }

    /// Domain-aware element logarithm of the stored rotor (contract §2).
    ///
    /// The logarithm is the log of the **stored value** `R = w + B + H`
    /// (`w` scalar, `B` grade 2, `H` grade >= 4), with the routing pinned by
    /// the review loop:
    ///
    /// - `B = 0`, `H = 0` (scalar-only): `w = 1` -> exactly zero;
    ///   `w > 0` -> `ln w`; `w = 0` (including `±0`) and non-finite `w` ->
    ///   [`RotorError::NotALogDomain`]; `w < 0` ->
    ///   [`RotorError::BranchAmbiguous`].
    /// - `H != 0` -> [`RotorError::Uncertified`] (multi-plane).
    /// - `H = 0`, `B != 0`: `B²` is classified exactly by the W03 dyadic square
    ///   (`exact_x2`), with no floating-point threshold:
    ///   * elliptic `B² = −β² < 0`:
    ///     `log R = ½·ln(w² + β²) + atan2(β, w)·(B/β)`;
    ///   * hyperbolic `B² = +β² > 0`: accepted iff `w > 0` and the certified
    ///     reverse norm `N = w² − β² > 0`; evaluated with the unified
    ///     `q = (w+β)²/N` small-`δ`/large-`q` route;
    ///   * parabolic `B² = 0`, `B != 0`: `log R = ln w + B/w` for `w > 0`;
    ///   * non-scalar `B²` -> [`RotorError::Uncertified`].
    ///
    /// Every scalar stage is evaluated on exact dyadic rationals with symbolic
    /// mantissa/exponent decomposition, so `w² + β²` is never formed in `f64`
    /// (it would overflow for `w = β = 1e154` though the true scale is
    /// `354.944677911363…`).
    pub fn logarithm(&self) -> Result<Multivector<P, Q, R>, RotorError> {
        let w = self.multivector.get(0);
        let basis_count = Multivector::<P, Q, R>::BASIS_COUNT;

        // Classify the grade-0/2/other coefficients exactly.
        let mut has_bivector = false;
        let mut has_higher = false;
        for index in 1..basis_count {
            let coefficient = self.multivector.get(index);
            if coefficient == 0.0 {
                continue;
            }
            if index.count_ones() == 2 {
                has_bivector = true;
            } else {
                has_higher = true;
            }
        }

        // §2b: any nonzero non-grade-0/2 content is uncertified. (The contract
        // names grade >= 4; odd grades cannot occur in a constructed even rotor
        // but are rejected here as a safe superset.)
        if has_higher {
            return Err(RotorError::Uncertified);
        }

        // §2a: scalar-only.
        if !has_bivector {
            return Self::logarithm_scalar(w);
        }

        // §2c requires a finite scalar part throughout.
        if !w.is_finite() {
            return Err(RotorError::NotALogDomain);
        }

        let bivector = self.multivector.grade_projection(2);
        let square = exact_x2(&bivector);
        if square.is_unsupported() || square.has_non_finite_source() || !square.is_exactly_scalar()
        {
            return Err(RotorError::Uncertified);
        }
        let (sign, beta) = square.scalar_sign_and_angle();

        if sign < 0 {
            Self::logarithm_elliptic(w, beta, &bivector)
        } else if sign > 0 {
            self.logarithm_hyperbolic(w, beta, &bivector)
        } else {
            Self::logarithm_parabolic(w, &bivector)
        }
    }

    /// §2a scalar-only routing.
    fn logarithm_scalar(w: f64) -> Result<Multivector<P, Q, R>, RotorError> {
        if !w.is_finite() {
            return Err(RotorError::NotALogDomain);
        }
        if w == 0.0 {
            // Includes `±0.0`; reachable from the compose domain exits.
            return Err(RotorError::NotALogDomain);
        }
        if w == 1.0 {
            return Ok(Multivector::zero());
        }
        if w < 0.0 {
            return Err(RotorError::BranchAmbiguous);
        }
        Ok(Multivector::scalar(w.ln()))
    }

    /// §2c elliptic logarithm: `R = w + B`, `B² = −β² < 0`.
    fn logarithm_elliptic(
        w: f64,
        beta: f64,
        bivector: &Multivector<P, Q, R>,
    ) -> Result<Multivector<P, Q, R>, RotorError> {
        let beta_exact = ExactPositive::from_f64(beta).ok_or(RotorError::NotALogDomain)?;
        let beta_sq = beta_exact
            .mul(&beta_exact)
            .ok_or(RotorError::NotALogDomain)?;
        let scale = if w == 0.0 {
            beta_sq.ln_half()
        } else {
            let w_exact = ExactPositive::from_f64(w.abs()).ok_or(RotorError::NotALogDomain)?;
            let w_sq = w_exact.mul(&w_exact).ok_or(RotorError::NotALogDomain)?;
            w_sq.add(&beta_sq)
                .ok_or(RotorError::NotALogDomain)?
                .ln_half()
        };
        let theta = beta.atan2(w);
        let mut log = bivector * (theta / beta);
        log.set(0, scale);
        if log.as_slice().iter().all(|c| c.is_finite()) {
            Ok(log)
        } else {
            Err(RotorError::NotALogDomain)
        }
    }

    /// §2c hyperbolic logarithm: `R = w + B`, `B² = +β² > 0`.
    fn logarithm_hyperbolic(
        &self,
        w: f64,
        beta: f64,
        bivector: &Multivector<P, Q, R>,
    ) -> Result<Multivector<P, Q, R>, RotorError> {
        if w <= 0.0 {
            return Err(RotorError::NotALogDomain);
        }
        let norm =
            exact_reverse_norm_positive(&self.multivector).ok_or(RotorError::NotALogDomain)?;
        let w_exact = ExactPositive::from_f64(w).ok_or(RotorError::NotALogDomain)?;
        let beta_exact = ExactPositive::from_f64(beta).ok_or(RotorError::NotALogDomain)?;
        // Exact `(w + β)²` and its difference from the certified `N`; no f64
        // materialization of the sum, the square, or the quotient.
        let wb = w_exact.add(&beta_exact).ok_or(RotorError::NotALogDomain)?;
        let wb_sq = wb.mul(&wb).ok_or(RotorError::NotALogDomain)?;
        let delta_num = wb_sq.sub(&norm).ok_or(RotorError::NotALogDomain)?;
        let phi = match delta_num.cmp_abs(&norm) {
            Some(Ordering::Less) => {
                // Cancellation-safe small rapidity: exact `δ = δ_num/N`
                // (one quotient rounding) then `½·ln1p(δ)`.
                let delta = delta_num.ratio_to_f64(&norm);
                0.5 * delta.ln_1p()
            }
            _ => {
                // `q = (w+β)²/N >= 2`: `½·ln q` with the numerator and
                // denominator logs combined through a single shared
                // mantissa/exponent decomposition.
                0.5 * wb_sq.ln_ratio(&norm)
            }
        };
        let scale = norm.ln_half();
        let mut log = bivector * (phi / beta);
        log.set(0, scale);
        if log.as_slice().iter().all(|c| c.is_finite()) {
            Ok(log)
        } else {
            Err(RotorError::NotALogDomain)
        }
    }

    /// §2c parabolic logarithm: `R = w + B`, `B² = 0`, `B != 0`.
    fn logarithm_parabolic(
        w: f64,
        bivector: &Multivector<P, Q, R>,
    ) -> Result<Multivector<P, Q, R>, RotorError> {
        if w <= 0.0 {
            return Err(RotorError::NotALogDomain);
        }
        let mut log = bivector * (1.0 / w);
        log.set(0, w.ln());
        if log.as_slice().iter().all(|c| c.is_finite()) {
            Ok(log)
        } else {
            Err(RotorError::NotALogDomain)
        }
    }

    /// Raise the rotor to an integer power (contract §2d).
    ///
    /// `0` -> identity; `1` -> a bit-exact clone; `−1` -> the stored-element
    /// inverse (bit-exact relative to [`Self::inverse`]); other exponents use
    /// the certified logarithm route `exp(n·log R)`. An uncertified
    /// (multi-plane) rotor falls back to left-to-right binary exponentiation
    /// with [`Self::compose`] and the certified-`N` gate at each composition.
    pub fn power_integer(&self, n: i64) -> Result<Self, RotorError> {
        match n {
            0 => Ok(Self::identity()),
            1 => Ok(Self {
                multivector: self.multivector.clone(),
            }),
            -1 => self.inverse(),
            _ => match self.logarithm() {
                Ok(log_r) => {
                    let scaled = log_r * (n as f64);
                    let result = scaled.exp()?;
                    Self::certify_exact_scalar_n(result)
                }
                Err(RotorError::Uncertified) => self.binary_power(n),
                Err(error) => Err(error),
            },
        }
    }

    /// Left-to-right binary exponentiation with `compose` (contract §2d).
    ///
    /// The chain for exponent `k >= 2` is the standard square-and-multiply
    /// chain: one leading-factor copy, then for each remaining bit one squaring
    /// and, when the bit is set, one multiply. For `k = 7` this is four
    /// compositions (`R²`, `R³`, `R⁶`, `R⁷`). Each composition is certified
    /// (`N > 0`) so a representability-limit collapse is an honest
    /// [`RotorError::Degenerate`].
    fn binary_power(&self, n: i64) -> Result<Self, RotorError> {
        let negative = n < 0;
        let magnitude = n.unsigned_abs();
        let base = Self {
            multivector: self.multivector.clone(),
        };
        let bits = 64 - magnitude.leading_zeros();
        let mut result = Self {
            multivector: base.multivector.clone(),
        };
        for bit in (0..bits - 1).rev() {
            result = result.checked_compose(&result)?;
            if (magnitude >> bit) & 1 == 1 {
                result = result.checked_compose(&base)?;
            }
        }
        if negative {
            result.inverse()
        } else {
            Ok(result)
        }
    }

    /// `compose` followed by the binary-exponentiation collapse gate.
    ///
    /// Contract §2d requires that every checked rotor-producing operation
    /// certify an output scalar `N > 0`. The gate is the **full** exact
    /// reverse-norm certificate, not merely the sign of the scalar part: a
    /// multi-plane product whose exact `R·R†` has any nonzero non-scalar
    /// content (coefficient-rounding residue) is [`RotorError::Degenerate`],
    /// matching what its own `inverse()` would report (CORE-W04 R1-F5).
    fn checked_compose(&self, other: &Self) -> Result<Self, RotorError> {
        let composed = Self {
            multivector: self.multivector.geometric_product(&other.multivector),
        };
        match exact_reverse_norm(&composed.multivector) {
            ReverseNorm::Positive => Ok(composed),
            _ => Err(RotorError::Degenerate),
        }
    }

    /// Raise the rotor to a real power (contract §2d).
    ///
    /// A `t` that is exactly integral in the `i64` range routes through
    /// [`Self::power_integer`]; otherwise `exp(t·log R)` through the W03
    /// pipeline. Logarithm domain failures propagate directly; exponential
    /// failures arrive as [`RotorError::Exp`].
    pub fn power(&self, t: f64) -> Result<Self, RotorError> {
        // `2^63` is excluded: it is not an `i64`, and the saturating cast would
        // silently clamp it.
        const I64_MIN_AS_F64: f64 = -9_223_372_036_854_775_808.0;
        const TWO_POW_63: f64 = 9_223_372_036_854_775_808.0;
        if t.fract() == 0.0 && (I64_MIN_AS_F64..TWO_POW_63).contains(&t) {
            return self.power_integer(t as i64);
        }
        let log_r = self.logarithm()?;
        let scaled = log_r * t;
        let result = scaled.exp()?;
        Self::certify_exact_scalar_n(result)
    }
}

/// 3D-specific rotor helpers (contract §4.4).
///
/// Axis-angle and rotation-matrix construction are inherently index-3D: the
/// bivector is the Hodge dual of the axis vector. They are therefore defined
/// only for `Rotor<3,0,0>`; generic callers on other signatures are a compile
/// error and must use the dimension-general [`Rotor::from_bivector`] route.
impl Rotor<3, 0, 0> {
    /// Create a rotor from a 3D axis-angle representation.
    ///
    /// The axis is normalized and dualized to the plane bivector
    /// `(a3·e12 − a2·e13 + a1·e23)`, then exponentiated with the same
    /// right-handed convention as [`Rotor::from_bivector`].
    ///
    /// Returns [`RotorError::NullVector`] for a null (zero) axis,
    /// [`RotorError::NotNormalizable`] for a negative-square or non-finite
    /// axis, and otherwise the exponential and normalization errors of
    /// [`Rotor::from_multivector_bivector`].
    pub fn from_axis_angle(axis: &Vector<3, 0, 0>, angle: f64) -> Result<Self, RotorError> {
        let axis_mv = axis.as_multivector();
        // Range-safe exact scalar square: a direct `norm_squared()` overflows
        // for `|axis| ~ 1e200` (spurious `Ok(identity)`) and underflows for
        // `|axis| ~ 1e-200` (spurious `Err(NullVector)`). `exact_x2` computes
        // `sigma(x) = x·x` on the exact dyadics, so normalization happens
        // before any null/finite decision (CORE-W04 R1-F7).
        let (sign, beta) = Self::certified_vector_square(axis_mv)?;
        if sign == 0 {
            return Err(RotorError::NullVector);
        }
        if sign < 0 {
            return Err(RotorError::NotNormalizable);
        }
        // `sign != 0` means the exact magnitude is positive, so `beta > 0`
        // (possibly subnormal); no pre-normalization null check can trip.
        let normalized_axis = axis_mv / beta;

        // For a 3D axis (a1, a2, a3), the corresponding bivector is
        // (a3·e12 − a2·e13 + a1·e23).
        let a1 = normalized_axis.get(1); // e1 component
        let a2 = normalized_axis.get(2); // e2 component
        let a3 = normalized_axis.get(4); // e3 component

        let mut bivector = Multivector::<3, 0, 0>::zero();
        let _ = bivector.set_bivector_component(0, a3); // e12 component
        let _ = bivector.set_bivector_component(1, -a2); // e13 component
        let _ = bivector.set_bivector_component(2, a1); // e23 component

        Self::from_multivector_bivector(&bivector, angle)
    }

    /// Convert the rotor to a 3×3 rotation matrix.
    ///
    /// Computes the matrix `M` such that `M·v = R·v·R†` for column vectors.
    /// Each column `j` is the image of basis vector `e_j` under the rotation.
    pub fn to_rotation_matrix(&self) -> [[f64; 3]; 3] {
        // Compute rotated basis vectors.
        let cols: Vec<_> = (0..3)
            .map(|j| {
                let basis = Multivector::<3, 0, 0>::basis_vector(j);
                // In-domain 3D rotation: the certified rotor acts.
                self.apply(&basis).expect("certified rotor acts")
            })
            .collect();

        // Build matrix: M[i][j] = (rotated e_j).component(i)
        let mut matrix = [[0.0; 3]; 3];
        for (i, row) in matrix.iter_mut().enumerate() {
            for (j, entry) in row.iter_mut().enumerate() {
                *entry = cols[j].vector_component(i);
            }
        }

        matrix
    }
}

/// Grade involution `ĉ` (contract §3b): grade-k coefficients scaled by
/// `(−1)^k`.
fn grade_involution<const P: usize, const Q: usize, const R: usize>(
    a: &Multivector<P, Q, R>,
) -> Multivector<P, Q, R> {
    let mut result = Multivector::zero();
    for index in 0..Multivector::<P, Q, R>::BASIS_COUNT {
        let sign = if index.count_ones() % 2 == 0 {
            1.0
        } else {
            -1.0
        };
        result.set(index, sign * a.get(index));
    }
    result
}

/// Validate a reflection normal: certified pure grade-1 with `n·n = ±1`
/// exactly. Precedence: `NotAVector` before `NotUnitNormal` (contract §3a/§3b).
fn certified_unit_normal<const P: usize, const Q: usize, const R: usize>(
    n: &Multivector<P, Q, R>,
) -> Result<f64, RotorError> {
    if has_foreign_grade(n, 1) {
        return Err(RotorError::NotAVector);
    }
    exact_reverse_norm_unit(n).ok_or(RotorError::NotUnitNormal)
}

/// Reflection through the hyperplane with unit normal `n` (contract §3b).
///
/// `reflect(n, A) = n · ĉ(A) · n⁻¹`, where `ĉ` is the grade involution.
/// `n` must be a certified pure vector with `n·n = ±1`; `n⁻¹ = n/(n·n)` is
/// exact because the certified square is `±1`. Errors: [`RotorError::NotAVector`]
/// for a non-vector normal, [`RotorError::NotUnitNormal`] otherwise.
pub fn reflect<const P: usize, const Q: usize, const R: usize>(
    n: &Multivector<P, Q, R>,
    a: &Multivector<P, Q, R>,
) -> Result<Multivector<P, Q, R>, RotorError> {
    let square = certified_unit_normal(n)?;
    let n_inverse = n * square; // n / (n·n), with n·n = ±1 exactly
    let involuted = grade_involution(a);
    Ok(n.geometric_product(&involuted)
        .geometric_product(&n_inverse))
}

/// Reflection through the line spanned by the unit vector `n` (contract §3c).
///
/// `reflect_line(n, A) = n · A · n⁻¹` — the versor sandwich (no grade
/// involution), the correct outermorphism of the line map. The same certified
/// pure-vector `n·n = ±1` normal validation as [`reflect`] applies.
pub fn reflect_line<const P: usize, const Q: usize, const R: usize>(
    n: &Multivector<P, Q, R>,
    a: &Multivector<P, Q, R>,
) -> Result<Multivector<P, Q, R>, RotorError> {
    let square = certified_unit_normal(n)?;
    let n_inverse = n * square; // n / (n·n), with n·n = ±1 exactly
    Ok(n.geometric_product(a).geometric_product(&n_inverse))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rotor_90_degrees() {
        let e1 = Vector::<3, 0, 0>::e1();
        let _e2 = Vector::<3, 0, 0>::e2();
        let e12 = Bivector::<3, 0, 0>::e12();

        // Create 90-degree rotation in e1-e2 plane
        // Bounded Euclidean bivector with a small finite angle: the exact
        // bivector route cannot overflow, so `.expect` is justified.
        let rotor = Rotor::from_bivector(&e12, core::f64::consts::PI / 2.0)
            .expect("bounded Euclidean bivector exponential");

        // Apply to e1, should get e2
        let rotated = rotor
            .apply_to_vector(&e1)
            .expect("certified rotor acts on a vector");
        assert!((rotated.mv.vector_component(1) - 1.0).abs() < 1e-10); // Should be e2
        assert!(rotated.mv.vector_component(0).abs() < 1e-10); // e1 component should be ~0
    }

    #[test]
    fn test_rotor_composition() {
        let e12 = Bivector::<3, 0, 0>::e12();

        // Two 45-degree rotations
        let rotor1 = Rotor::from_bivector(&e12, core::f64::consts::PI / 4.0)
            .expect("bounded Euclidean bivector exponential");
        let rotor2 = Rotor::from_bivector(&e12, core::f64::consts::PI / 4.0)
            .expect("bounded Euclidean bivector exponential");

        // Compose them
        let composed = rotor1.compose(&rotor2);

        // Should equal a single 90-degree rotation
        let rotor90 = Rotor::from_bivector(&e12, core::f64::consts::PI / 2.0)
            .expect("bounded Euclidean bivector exponential");

        let diff = composed.as_multivector() - rotor90.as_multivector();
        assert!(diff.coefficient_norm() < 1e-10);
    }

    #[test]
    fn test_reflection() {
        let e1 = Vector::<3, 0, 0>::e1();
        let mut v = Multivector::<3, 0, 0>::zero();
        v.set_vector_component(0, 1.0); // e1
        v.set_vector_component(1, 1.0); // e2

        // Line map across e1 (should negate e2 component): the migrated
        // baseline assertion, now expressed with `reflect_line`.
        let reflected = reflect_line(&e1.mv, &v).expect("e1 is a certified unit normal");
        assert!((reflected.vector_component(0) - 1.0).abs() < 1e-10); // e1 unchanged
        assert!((reflected.vector_component(1) + 1.0).abs() < 1e-10); // e2 negated

        // CORE-W04 §6 item 6 pin, verbatim: `reflect_line(e1, e1) = +e1`.
        let fixed = reflect_line(&e1.mv, &e1.mv).expect("e1 is a certified unit normal");
        assert_eq!(fixed.as_slice()[1], 1.0);
        assert!(fixed
            .as_slice()
            .iter()
            .enumerate()
            .all(|(i, &c)| i == 1 || c == 0.0));
    }

    // CORE-W04 §1.4b negative-sheet inverse battery.
    //
    // The negative-sheet element `−5/4 + (3/4)e12` cannot be produced by the
    // identity-component constructors (its scalar is negative), so the unit
    // test constructs it through the private stored field. Its certified
    // `N = R·R† = 1` exactly, so `inverse()` is the bare reverse and must not
    // depend on the (negative-sheet-rejecting) logarithm.
    #[test]
    fn test_negative_sheet_stored_inverse_uses_reverse_over_norm() {
        let mut stored = Multivector::<1, 1, 0>::zero();
        stored.set(0, -1.25); // scalar ω = −5/4
        stored.set(3, 0.75); // e12 = (3/4)e12
        let rotor = Rotor {
            multivector: stored,
        };

        let inverse = rotor
            .inverse()
            .expect("negative-sheet N = 1 > 0 is invertible");
        assert_eq!(inverse.as_slice()[0], -1.25);
        assert_eq!(inverse.as_slice()[3], -0.75);

        // The action gate accepts the same stored element.
        let acted = rotor
            .apply(&Multivector::scalar(1.0))
            .expect("negative-sheet element has certified N > 0");
        assert!(acted.as_slice().iter().all(|c| c.is_finite()));
    }

    // ==================================================================
    // CORE-W04 stage 2 batteries that need the private stored field.
    // ==================================================================

    /// Largest coefficient-wise ulp distance between two slices.
    fn ulp_distance(a: &[f64], b: &[f64]) -> u64 {
        fn key(x: f64) -> u64 {
            let bits = x.to_bits();
            if bits & (1u64 << 63) != 0 {
                !bits
            } else {
                bits | (1u64 << 63)
            }
        }
        a.iter()
            .zip(b.iter())
            .map(|(&x, &y)| key(x).abs_diff(key(y)))
            .max()
            .unwrap_or(0)
    }

    #[test]
    fn test_logarithm_scalar_branches() {
        // w = 1 -> exactly zero.
        let one = Rotor::<1, 0, 0> {
            multivector: Multivector::scalar(1.0),
        };
        assert!(one
            .logarithm()
            .expect("scalar 1 is certified")
            .as_slice()
            .iter()
            .all(|&c| c == 0.0));

        // w = 4 -> ln 4 = 2·ln2 (disclosed: exp(ln w) is rounding-bounded).
        let four = Rotor::<1, 0, 0> {
            multivector: Multivector::scalar(4.0),
        };
        assert_eq!(
            four.logarithm().expect("scalar 4 is certified").get(0),
            2.0 * 2f64.ln()
        );

        // w = 0 including signed zero -> NotALogDomain.
        for w in [0.0, -0.0] {
            let zero = Rotor::<1, 0, 0> {
                multivector: Multivector::scalar(w),
            };
            assert!(matches!(zero.logarithm(), Err(RotorError::NotALogDomain)));
        }

        // w < 0 -> BranchAmbiguous.
        let negative = Rotor::<1, 0, 0> {
            multivector: Multivector::scalar(-3.0),
        };
        assert!(matches!(
            negative.logarithm(),
            Err(RotorError::BranchAmbiguous)
        ));

        // Non-finite w -> NotALogDomain.
        for w in [f64::INFINITY, f64::NEG_INFINITY, f64::NAN] {
            let bad = Rotor::<1, 0, 0> {
                multivector: Multivector::scalar(w),
            };
            assert!(matches!(bad.logarithm(), Err(RotorError::NotALogDomain)));
        }
    }

    #[test]
    fn test_negative_sheet_logarithm_rejects() {
        let mut stored = Multivector::<1, 1, 0>::zero();
        stored.set(0, -1.25);
        stored.set(3, 0.75);
        let rotor = Rotor {
            multivector: stored,
        };
        assert!(matches!(rotor.logarithm(), Err(RotorError::NotALogDomain)));
    }

    #[test]
    fn test_cl6_i_rotor_logarithm_is_uncertified() {
        // The Cl(6,0,0) element (1 + I)/√2 is a valid versor (N = 1) but has
        // grade-6 content, so it is outside the certified single-plane class.
        let inv_sqrt2 = core::f64::consts::FRAC_1_SQRT_2;
        let mut stored = Multivector::<6, 0, 0>::zero();
        stored.set(0, inv_sqrt2);
        stored.set(63, inv_sqrt2); // I = e123456
        let rotor = Rotor {
            multivector: stored,
        };
        assert!(matches!(rotor.logarithm(), Err(RotorError::Uncertified)));
    }

    #[test]
    fn test_logarithm_elliptic_extreme_scale_is_range_safe() {
        // R = 1e154 + 1e154·e12 in Cl(3,0,0): B² = −1e308, so the scale is
        // ½·ln(2·(1e154)²) = 354.944677911363… ; a direct f64 w² + β² is inf.
        let mut stored = Multivector::<3, 0, 0>::zero();
        stored.set(0, 1e154);
        stored.set(3, 1e154); // e12
        let rotor = Rotor {
            multivector: stored,
        };
        let log = rotor.logarithm().expect("elliptic certified");
        assert!(
            (log.get(0) - 354.944677911363).abs() < 1e-9,
            "scale {}",
            log.get(0)
        );
        // w = β gives θ = atan2(β, w) = π/4.
        assert!((log.get(3) - core::f64::consts::FRAC_PI_4).abs() < 1e-12);
    }

    #[test]
    fn test_logarithm_hyperbolic_extreme_rapidity_is_range_safe() {
        // w = 1e308, β = nextafter(1e308, 0): w² and β² overflow f64 but the
        // certified exact N = w² − β² is positive and the rapidity is finite.
        let w: f64 = 1e308;
        let beta = f64::from_bits(w.to_bits() - 1);
        let mut stored = Multivector::<1, 1, 0>::zero();
        stored.set(0, w);
        stored.set(3, beta); // e12, e12² = +1
        let rotor = Rotor {
            multivector: stored,
        };
        let log = rotor.logarithm().expect("hyperbolic certified");
        assert!(
            (log.get(3) - 18.42172174950956).abs() < 1e-9,
            "rapidity {}",
            log.get(3)
        );
    }

    #[test]
    fn test_logarithm_hyperbolic_mixed_component_rapidity_is_28_ln2() {
        // Exact fixture in Cl(2,1,0): R = 2²⁷ + 2²⁷e13 + e12, so
        // B² = 2⁵⁴·e13² + e12² = 2⁵⁴ − 1, w = 2²⁷, N = w² − B² = 1, and the
        // rapidity is atanh(√(2⁵⁴−1)/2²⁷) → ½·ln(2⁵⁶) = 28·ln2. The exact
        // fixture needs the private stored field (the exp/normalize route loses
        // its scale to a 1-ulp conditioning artifact).
        let mut stored = Multivector::<2, 1, 0>::zero();
        stored.set(0, 2f64.powi(27)); // w
        stored.set(3, 1.0); // e12
        stored.set(5, 2f64.powi(27)); // e13
        let rotor = Rotor {
            multivector: stored,
        };
        let log = rotor.logarithm().expect("hyperbolic certified");
        let expected = 28.0 * 2f64.ln();
        assert!(
            (log.get(5) - expected).abs() < 1e-9,
            "rapidity {} vs {expected}",
            log.get(5)
        );
        // The e12 direction component is φ·2⁻²⁷ exactly.
        assert!((log.get(3) - expected * 2f64.powi(-27)).abs() < 1e-9);
    }

    #[test]
    fn test_power_integer_r32_chain_is_degenerate() {
        // The exact dyadic boost `5/4 + (3/4)e12` squared five times stores
        // `[2³¹, 2³¹]` with certified `N = 0`; the closed-form power agrees.
        let mut stored = Multivector::<1, 1, 0>::zero();
        stored.set(0, 1.25);
        stored.set(3, 0.75);
        let r0 = Rotor {
            multivector: stored,
        };
        let mut r32 = Rotor {
            multivector: r0.multivector.clone(),
        };
        for _ in 0..5 {
            r32 = r32.compose(&r32);
        }
        assert_eq!(r32.as_slice()[0], 2f64.powi(31));
        assert_eq!(r32.as_slice()[3], 2f64.powi(31));

        assert!(matches!(r32.inverse(), Err(RotorError::Degenerate)));
        assert!(matches!(
            r32.apply(&Multivector::scalar(1.0)),
            Err(RotorError::Degenerate)
        ));
        assert!(matches!(r0.power_integer(32), Err(RotorError::Degenerate)));
    }

    #[test]
    fn test_logarithm_round_trip_witness_within_17_ulp() {
        // R8-verified witness: w = 1, b = nextafter(1,0). The exact scalar
        // logarithms are scale −18.021826694558577 and rapidity
        // 18.714973875118524; the composite chain is conditioning-bounded.
        let beta = f64::from_bits(1.0f64.to_bits() - 1);
        let mut stored = Multivector::<1, 1, 0>::zero();
        stored.set(0, 1.0);
        stored.set(3, beta);
        let rotor = Rotor {
            multivector: stored,
        };
        let log = rotor.logarithm().expect("hyperbolic certified");
        assert!((log.get(0) - (-18.021826694558577)).abs() < 1e-12);
        assert!((log.get(3) - 18.714973875118524).abs() < 1e-12);

        let recovered = log.exp().expect("W03 exponential");
        let distance = ulp_distance(recovered.as_slice(), rotor.as_slice());
        assert!(distance <= 17, "witness {distance} ulp");
        // R8-verified high-precision result for this fixture.
        assert!((recovered.get(0) - 1.000000000000002).abs() <= 2.3e-15);
        assert!((recovered.get(3) - 1.0000000000000018).abs() <= 4.0e-15);
    }

    // ==================================================================
    // CORE-W04 stage 3 candidate identities (contract §3a.3).
    // ==================================================================

    /// `C = 1 + s·b̂â` satisfies `Câ = b̂C` and `C = s·(b̂+â)·â` exactly.
    fn assert_candidate_identities<const P: usize, const Q: usize, const R: usize>(
        a_hat: &Multivector<P, Q, R>,
        b_hat: &Multivector<P, Q, R>,
        s: i32,
    ) {
        let candidate = Rotor::<P, Q, R>::direction_candidate(a_hat, b_hat, s);
        let left = candidate.geometric_product(a_hat);
        let right = b_hat.geometric_product(&candidate);
        assert_eq!(left.as_slice(), right.as_slice(), "Câ = b̂C");

        let factorized = (b_hat + a_hat).geometric_product(a_hat) * (s as f64);
        assert_eq!(candidate.as_slice(), factorized.as_slice(), "C = s·(b̂+â)·â");
    }

    #[test]
    fn test_direction_candidate_identities_positive_sign() {
        // Cl(2,0,0), s = +1: â = e1, b̂ = e2.
        let a_hat = Multivector::<2, 0, 0>::basis_vector(0);
        let b_hat = Multivector::<2, 0, 0>::basis_vector(1);
        assert_candidate_identities(&a_hat, &b_hat, 1);
    }

    #[test]
    fn test_direction_candidate_identities_negative_sign() {
        // Cl(0,2,0), s = −1: â = e1, b̂ = e2.
        let a_hat = Multivector::<0, 2, 0>::basis_vector(0);
        let b_hat = Multivector::<0, 2, 0>::basis_vector(1);
        assert_candidate_identities(&a_hat, &b_hat, -1);
    }

    #[test]
    fn test_slerp_coefficient_dot_tie_keeps_second_representative() {
        // A = 1, B = e12 in Cl(2,0,0): the coefficient dot is exactly zero, so
        // the tie keeps B (no negation).
        let one = Rotor::<2, 0, 0> {
            multivector: Multivector::scalar(1.0),
        };
        let mut e12_mv = Multivector::<2, 0, 0>::zero();
        e12_mv.set(3, 1.0);
        let e12 = Rotor::<2, 0, 0> {
            multivector: e12_mv,
        };

        let endpoint = one.slerp(&e12, 1.0).expect("endpoint representative");
        assert_eq!(endpoint.as_slice(), e12.as_slice());

        // e12^(1/2) = (1 + e12)/√2, so the e12 coefficient is positive.
        let mid = one.slerp(&e12, 0.5).expect("interior");
        assert!(mid.as_slice()[3] > 0.0);
    }

    // ==================================================================
    // CORE-W04 R1 remediation unit batteries.
    // ==================================================================

    #[test]
    fn test_hyperbolic_log_ratio_is_correctly_rounded() {
        // R1-F3: the stored element `1 + beta e12` in Cl(1,1,0) with
        // beta = 0.07081442692123036 has exact delta = delta_num / N. The
        // single-division route gives the correctly rounded rapidity
        // ½·ln1p(delta) = 0.07093315499237553 (the old two-significand route
        // gave 0.07093315499237551).
        let mut stored = Multivector::<1, 1, 0>::zero();
        stored.set(0, 1.0);
        stored.set(3, 0.07081442692123036);
        let rotor = Rotor {
            multivector: stored,
        };
        let log = rotor.logarithm().expect("hyperbolic certified");
        assert_eq!(log.get(3), 0.07093315499237553);
    }

    #[test]
    fn test_logarithm_above_six_dimensions_is_uncertified() {
        // R1-F1: a bivector-bearing rotor above n = 6 must route the exact
        // square to `Uncertified` rather than panicking in the fixed table.
        let mut stored = Multivector::<7, 0, 0>::zero();
        stored.set(3, 1.0); // e12
        let rotor = Rotor {
            multivector: stored,
        };
        assert!(matches!(rotor.logarithm(), Err(RotorError::Uncertified)));
    }

    #[test]
    fn test_dyadic_multiphane_binary_power_chain_is_exact() {
        // Cl(2,2,0): R = (5/4 + 3/4 e12)(5/4 + 3/4 e34) is a grade-4 multi-plane
        // element with exactly scalar N = 1 (all coefficients are exact
        // dyadics, so the versor identity holds exactly). powers route through
        // the binary-composition fallback and the full certificate accepts
        // every boundary (R1-F5's contrast case to the rounded fixture).
        let mut r12 = Multivector::<2, 2, 0>::zero();
        r12.set(0, 1.25);
        r12.set(3, 0.75); // e12
        let mut r34 = Multivector::<2, 2, 0>::zero();
        r34.set(0, 1.25);
        r34.set(12, 0.75); // e34
        let product = r12.geometric_product(&r34);
        let rotor = Rotor {
            multivector: product.clone(),
        };
        // Grade-4 content makes the logarithm uncertified -> binary fallback.
        assert!(matches!(rotor.logarithm(), Err(RotorError::Uncertified)));

        let raw_square = product.geometric_product(&product);
        let power_two = rotor.power_integer(2).expect("exact scalar-N chain");
        assert_eq!(power_two.as_slice(), raw_square.as_slice());

        // Exact composition law: power(3) ∘ power(4) == power(7).
        let p3 = rotor.power_integer(3).expect("p3");
        let p4 = rotor.power_integer(4).expect("p4");
        let p7 = rotor.power_integer(7).expect("p7");
        let lhs = p3.multivector.geometric_product(&p4.multivector);
        assert_eq!(lhs.as_slice(), p7.as_slice());
    }
}
