// SPDX-License-Identifier: MIT OR Apache-2.0

//! Agent-first discovery and planning for the Amari mathematical ecosystem.
//!
//! `amari-discovery` provides the typed engine behind the `amari` command.
//! Its runtime authority is read-only: it may inspect projects, recommend
//! capabilities, construct plans, and run registered bounded probes.
