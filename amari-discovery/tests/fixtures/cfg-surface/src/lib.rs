// SPDX-License-Identifier: MIT OR Apache-2.0

//! Fixture crate for cfg-gate tests (Task 5C1).
//!
//! Exercises simple default/disabled features, all/any combinations,
//! inherited module gates, same-file cfg variants, and unsupported predicates.

// --- Simple feature-gated items ---

/// Always available — no cfg gate.
pub fn always_available() {}

/// Gated behind a feature that is enabled by default.
#[cfg(feature = "default_on")]
pub fn default_feature_gated() {}

/// Gated behind a feature that is not in the default closure.
#[cfg(feature = "opt_in")]
pub fn opt_in_feature_gated() {}

// --- all / any combinations ---

#[cfg(all(feature = "default_on", feature = "opt_in"))]
pub fn all_conjunction() {}

#[cfg(any(feature = "default_on", feature = "opt_in"))]
pub fn any_disjunction() {}

// --- not(...) ---

#[cfg(not(feature = "default_on"))]
pub fn not_default() {}

// --- Inherited module gate ---

#[cfg(feature = "default_on")]
pub mod gated_module {
    /// Child of a cfg-gated module — inherits the module gate.
    pub fn child_in_gated_module() {}
}

// --- Same-file cfg variants (same source file, different #[cfg]) ---

#[cfg(feature = "default_on")]
pub struct SameFileVariant {
    pub default_field: u8,
}

#[cfg(feature = "opt_in")]
pub struct SameFileVariant {
    pub opt_in_field: u8,
}

// --- Unsupported predicate (target_os) ---

#[cfg(target_os = "linux")]
pub fn target_os_linux() {}

// --- Unsupported bare predicate (unix) ---

#[cfg(unix)]
pub fn bare_unix_predicate() {}

// --- Supported with unsupported mixed: any(feature, target_arch) ---

#[cfg(any(feature = "default_on", target_arch = "wasm32"))]
pub fn mixed_known_and_unknown() {}

// --- Impl-block cfg gates (Task 5C1 fix) ---

pub struct Owner;

/// Impl block gated on feature="impl_gate" — both members must inherit it.
#[cfg(feature = "impl_gate")]
impl Owner {
    pub fn impl_gated_method(&self) {}
    pub const IMPL_GATED_CONST: u8 = 0;
}

/// Impl + member level cfg combined.
#[cfg(feature = "impl_gate")]
impl Owner {
    #[cfg(feature = "member_gate")]
    pub fn impl_and_member_gated_method(&self) {}
}

/// Impl block with unsupported predicate.
#[cfg(feature = "default_on")]
impl Owner {
    #[cfg(target_os = "linux")]
    pub fn impl_default_member_unsupported(&self) {}
}

/// Ungated inherent method for cross-check.
impl Owner {
    pub fn always_method(&self) {}
}
