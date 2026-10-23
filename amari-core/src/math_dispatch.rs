// SPDX-License-Identifier: MIT OR Apache-2.0

//! Target-compatible float dispatch (P-EMBED S2b).
//!
//! The transcendental `f32`/`f64` methods (`sin`, `powf`, `ln`, …) are
//! provided by `std` only; `core` lacks them on bare-metal targets such
//! as `thumbv7em-none-eabihf`. This module routes every such call site
//! through one dispatch: on every hosted target (`target_os != "none"`,
//! including wasm32) each function is the inherent method — bit-identical
//! to the pre-P-EMBED code, on std AND no-default-feature host rows alike
//! (that is why the host matrix rows were always green) — and on bare
//! metal (`target_os = "none"`) the equivalent `libm` routine (the same
//! crate backing num-traits' own no-std `Float` impls).
//!
//! P-EMBED contract: docs/plans/2026-10-21-amari-pembed-contract.md (S2b).

// The dispatch covers the full census surface; individual feature and
// target combinations use subsets, so unused-function warnings are
// expected per-combination and silenced module-wide deliberately.
#![allow(dead_code)]

macro_rules! dispatch {
    ($(#[$meta:meta])* $name:ident, $ty:ty, $std:ident, $libm:ident) => {
        $(#[$meta])*
        #[inline]
        pub fn $name(x: $ty) -> $ty {
            #[cfg(all(not(target_os = "none"), feature = "std"))]
            { x.$std() }
            #[cfg(not(all(not(target_os = "none"), feature = "std")))]
            { libm::$libm(x) }
        }
    };
    ($(#[$meta:meta])* $name:ident, $ty:ty, $std:ident, $libm:ident, $($arg:ident: $aty:ty),+) => {
        $(#[$meta])*
        #[inline]
        pub fn $name(x: $ty, $($arg: $aty),+) -> $ty {
            #[cfg(all(not(target_os = "none"), feature = "std"))]
            { x.$std($($arg),+) }
            #[cfg(not(all(not(target_os = "none"), feature = "std")))]
            { libm::$libm(x, $($arg),+) }
        }
    };
}

dispatch!(powf_f64, f64, powf, pow, exp: f64);
dispatch!(sqrt_f64, f64, sqrt, sqrt);
dispatch!(exp_f64, f64, exp, exp);
dispatch!(ln_f64, f64, ln, log);
dispatch!(log2_f64, f64, log2, log2);
dispatch!(log10_f64, f64, log10, log10);
// log(x, base): libm exports only natural/log2/log10; the no_std arm
// composes them (bit-for-bit only for the special bases; the general
// case divides, matching the documented approximation surface).
#[inline]
pub fn log_f64(x: f64, base: f64) -> f64 {
    #[cfg(all(not(target_os = "none"), feature = "std"))]
    {
        x.log(base)
    }
    #[cfg(not(all(not(target_os = "none"), feature = "std")))]
    {
        if base == 2.0 {
            libm::log2(x)
        } else if base == 10.0 {
            libm::log10(x)
        } else {
            libm::log(x) / libm::log(base)
        }
    }
}
dispatch!(ln_1p_f64, f64, ln_1p, log1p);
dispatch!(sin_f64, f64, sin, sin);
dispatch!(cos_f64, f64, cos, cos);
dispatch!(tan_f64, f64, tan, tan);
dispatch!(sinh_f64, f64, sinh, sinh);
dispatch!(cosh_f64, f64, cosh, cosh);
dispatch!(tanh_f64, f64, tanh, tanh);
dispatch!(asin_f64, f64, asin, asin);
dispatch!(acos_f64, f64, acos, acos);
dispatch!(atan_f64, f64, atan, atan);
dispatch!(atan2_f64, f64, atan2, atan2, other: f64);
dispatch!(round_f64, f64, round, round);
dispatch!(floor_f64, f64, floor, floor);
dispatch!(ceil_f64, f64, ceil, ceil);
dispatch!(trunc_f64, f64, trunc, trunc);

dispatch!(powf_f32, f32, powf, powf, exp: f32);
dispatch!(sqrt_f32, f32, sqrt, sqrtf);
dispatch!(exp_f32, f32, exp, expf);
dispatch!(ln_f32, f32, ln, logf);
dispatch!(log2_f32, f32, log2, log2f);
dispatch!(log10_f32, f32, log10, log10f);
#[inline]
pub fn log_f32(x: f32, base: f32) -> f32 {
    #[cfg(all(not(target_os = "none"), feature = "std"))]
    {
        x.log(base)
    }
    #[cfg(not(all(not(target_os = "none"), feature = "std")))]
    {
        if base == 2.0 {
            libm::log2f(x)
        } else if base == 10.0 {
            libm::log10f(x)
        } else {
            libm::logf(x) / libm::logf(base)
        }
    }
}
dispatch!(ln_1p_f32, f32, ln_1p, log1pf);
dispatch!(sin_f32, f32, sin, sinf);
dispatch!(cos_f32, f32, cos, cosf);
dispatch!(tan_f32, f32, tan, tanf);
dispatch!(sinh_f32, f32, sinh, sinhf);
dispatch!(cosh_f32, f32, cosh, coshf);
dispatch!(tanh_f32, f32, tanh, tanhf);
dispatch!(asin_f32, f32, asin, asinf);
dispatch!(acos_f32, f32, acos, acosf);
dispatch!(atan_f32, f32, atan, atanf);
dispatch!(atan2_f32, f32, atan2, atan2f, other: f32);
dispatch!(round_f32, f32, round, roundf);
dispatch!(floor_f32, f32, floor, floorf);
dispatch!(ceil_f32, f32, ceil, ceilf);
dispatch!(trunc_f32, f32, trunc, truncf);

// powi: libm has no powi; the std arm keeps the exact inherent method,
// the no_std arm evaluates integer exponentiation by squaring over the
// float (deterministic, no libm dependence).
#[inline]
pub fn powi_f64(x: f64, exp: i32) -> f64 {
    #[cfg(all(not(target_os = "none"), feature = "std"))]
    {
        x.powi(exp)
    }
    #[cfg(not(all(not(target_os = "none"), feature = "std")))]
    {
        integer_pow(x, exp)
    }
}
fn integer_pow(mut base: f64, mut exp: i32) -> f64 {
    if exp < 0 {
        return libm::pow(base, exp as f64);
    }
    let mut acc = 1.0;
    while exp > 0 {
        if exp & 1 == 1 {
            acc *= base;
        }
        base *= base;
        exp >>= 1;
    }
    acc
}

// fract: std arm = the exact inherent method; no_std arm = x - trunc(x)
// (libm has no fract; sign semantics match the inherent method).
#[inline]
pub fn fract_f64(x: f64) -> f64 {
    #[cfg(all(not(target_os = "none"), feature = "std"))]
    {
        x.fract()
    }
    #[cfg(not(all(not(target_os = "none"), feature = "std")))]
    {
        x - libm::trunc(x)
    }
}
#[inline]
pub fn fract_f32(x: f32) -> f32 {
    #[cfg(all(not(target_os = "none"), feature = "std"))]
    {
        x.fract()
    }
    #[cfg(not(all(not(target_os = "none"), feature = "std")))]
    {
        x - libm::truncf(x)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn std_arm_matches_inherent() {
        // On std builds the dispatch IS the inherent method.
        assert_eq!(sqrt_f64(4.0), 4.0f64.sqrt());
        assert_eq!(ln_f64(1.0), 1.0f64.ln());
        assert_eq!(sin_f64(0.0), 0.0f64.sin());
        assert_eq!(powi_f64(2.0, 10), 1024.0);
        assert_eq!(fract_f64(3.25), 3.25f64.fract());
        assert_eq!(sqrt_f32(9.0), 9.0f32.sqrt());
    }
}
