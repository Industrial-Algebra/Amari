// SPDX-License-Identifier: MIT OR Apache-2.0

//! Deterministic capability planning primitives.
//!
//! Planning starts with bounded candidate retrieval from the embedded semantic
//! catalog. Later stages expand these candidates through capability relations,
//! rank trade-offs, and normalize replayable plans.

mod recall;

pub use recall::{CandidateRetriever, RecallConfig, RetrievalSource, RetrievedCandidate};
