// SPDX-License-Identifier: MIT OR Apache-2.0

//! Framed SHA-256 digests and alpha-canonical term serialization.
//!
//! All relation authority identity uses framed digests: the frame is a
//! length-prefixed domain label, so identical payloads under different
//! frames never collide. Canonical term serialization renumbers
//! variables by first structural occurrence (preorder), so
//! alpha-equivalent terms hash identically regardless of caller names.

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use core::fmt;

use sha2::{Digest, Sha256};

use crate::error::{RewriteError, RewriteResult};
use crate::trs::Term;

/// A validated SHA-256 digest.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub struct Sha256Digest([u8; 32]);

impl Sha256Digest {
    /// Wrap raw digest bytes.
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Borrow the digest bytes.
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Framed digest: `SHA-256(u32le frame len || frame || payload)`.
    pub fn framed(frame: &str, payload: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        let frame_len = u32::try_from(frame.len()).unwrap_or(u32::MAX);
        hasher.update(frame_len.to_le_bytes());
        hasher.update(frame.as_bytes());
        hasher.update(payload);
        Self(hasher.finalize().into())
    }

    /// Canonical alpha-invariant digest of a term under `frame`.
    ///
    /// Variables are renumbered by first structural occurrence in
    /// preorder; symbols and variables occupy disjoint tags; arity and
    /// name lengths are explicit, so encodings are prefix-free.
    pub fn canonical_term(frame: &str, term: &Term) -> Self {
        let mut encoding = Vec::new();
        let mut variables: Vec<String> = Vec::new();
        encode_term(term, &mut variables, &mut encoding);
        Self::framed(frame, &encoding)
    }

    /// Canonical lowercase hex rendering.
    pub fn to_hex(&self) -> String {
        let mut out = String::with_capacity(64);
        for byte in self.0 {
            out.push(char::from_digit((byte >> 4) as u32, 16).unwrap_or('0'));
            out.push(char::from_digit((byte & 0x0f) as u32, 16).unwrap_or('0'));
        }
        out
    }

    /// Parse a canonical lowercase hex digest (exactly 64 characters).
    pub fn parse_hex(text: &str) -> RewriteResult<Self> {
        let invalid = |message: &str| RewriteError::InvalidDigest {
            message: String::from(message),
        };
        if text.len() != 64 {
            return Err(invalid("expected exactly 64 hex characters"));
        }
        let bytes = text.as_bytes();
        let mut digest = [0u8; 32];
        let (pairs, remainder) = bytes.as_chunks::<2>();
        debug_assert!(remainder.is_empty());
        for (index, pair) in pairs.iter().enumerate() {
            let high =
                hex_value(pair[0]).ok_or_else(|| invalid("non-hex or non-lowercase character"))?;
            let low =
                hex_value(pair[1]).ok_or_else(|| invalid("non-hex or non-lowercase character"))?;
            digest[index] = (high << 4) | low;
        }
        Ok(Self(digest))
    }
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

/// Preorder canonical encoding with alpha-renaming: variables become
/// indices by first occurrence; structure, arities, and symbol names
/// are explicit. Shared with constraint canonicalization.
///
/// Iterative (explicit worklist, children pushed in reverse so the
/// pop order is preorder): the byte output and first-occurrence
/// variable indexing match the former recursive encoding exactly.
pub(crate) fn encode_term(term: &Term, variables: &mut Vec<String>, out: &mut Vec<u8>) {
    let mut stack = Vec::from([term]);
    while let Some(node) = stack.pop() {
        match node {
            Term::Var(variable) => {
                let name = variable.to_string();
                let index = variables
                    .iter()
                    .position(|known| *known == name)
                    .unwrap_or_else(|| {
                        variables.push(name);
                        variables.len() - 1
                    });
                out.push(0x00);
                push_u32(out, index as u32);
            }
            Term::Sym(symbol, args) => {
                out.push(0x01);
                let name = symbol.to_string();
                push_u32(out, args.len() as u32);
                push_u32(out, name.len() as u32);
                out.extend_from_slice(name.as_bytes());
                for arg in args.iter().rev() {
                    stack.push(arg);
                }
            }
        }
    }
}

fn push_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

impl fmt::Display for Sha256Digest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

impl fmt::Debug for Sha256Digest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Sha256Digest({self})")
    }
}
