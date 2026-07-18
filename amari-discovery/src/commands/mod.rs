// SPDX-License-Identifier: MIT OR Apache-2.0

//! Typed command handlers for the `amari` CLI.
//!
//! Each subcommand is implemented in its own module. All handlers
//! return typed [`crate::Envelope`] payloads; rendering is centralized
//! in the render module.

pub mod discover;
pub mod plan;
pub mod recommend;
