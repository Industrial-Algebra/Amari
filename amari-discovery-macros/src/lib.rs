// SPDX-License-Identifier: MIT OR Apache-2.0

//! Bounded procedural macros for `amari-discovery` wire contracts.
//!
//! The derive added by this crate intentionally supports only the Serde DTO
//! shapes used by executable Amari probes. Unsupported shapes fail at compile
//! time so wire-contract authority cannot silently drift into an underspecified
//! schema representation.
