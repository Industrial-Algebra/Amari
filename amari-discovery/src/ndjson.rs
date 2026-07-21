// SPDX-License-Identifier: MIT OR Apache-2.0

//! Shared bounded newline-delimited JSON record framing.

use std::io::Write;

use serde::Serialize;

use crate::{DiscoveryError, DiscoveryResult};

/// Default maximum serialized bytes for one NDJSON object, excluding newline.
pub const DEFAULT_MAX_NDJSON_RECORD_BYTES: usize = 1024 * 1024;

/// A writer that emits one bounded complete JSON object per flushed line.
pub struct NdjsonWriter<W> {
    writer: W,
    max_record_bytes: usize,
}

impl<W: Write> NdjsonWriter<W> {
    /// Creates a writer with the one-mebibyte default record ceiling.
    ///
    /// # Errors
    ///
    /// Returns invalid input only if the compiled default ceiling is zero.
    pub fn new(writer: W) -> DiscoveryResult<Self> {
        Self::with_max_record_bytes(writer, DEFAULT_MAX_NDJSON_RECORD_BYTES)
    }

    /// Creates a writer with an explicit serialized-record ceiling.
    ///
    /// # Errors
    ///
    /// Returns [`DiscoveryError::InvalidInput`] when `max_record_bytes` is zero.
    pub fn with_max_record_bytes(writer: W, max_record_bytes: usize) -> DiscoveryResult<Self> {
        if max_record_bytes == 0 {
            return Err(DiscoveryError::InvalidInput(
                "NDJSON record limit must be greater than zero".to_owned(),
            ));
        }
        Ok(Self {
            writer,
            max_record_bytes,
        })
    }

    /// Serializes, bounds, writes, newline-terminates, and flushes one object.
    ///
    /// String newlines are escaped by the JSON serializer and therefore never
    /// split a record. The size limit is checked before any bytes are written.
    ///
    /// # Errors
    ///
    /// Returns a serialization error, a limit error before writing, or a typed
    /// I/O error when writing or flushing fails.
    pub fn write<T: Serialize>(&mut self, value: &T) -> DiscoveryResult<()> {
        let record = serde_json::to_vec(value)?;
        if record.len() > self.max_record_bytes {
            return Err(DiscoveryError::LimitExceeded(format!(
                "NDJSON record bytes {} exceed limit {}",
                record.len(),
                self.max_record_bytes
            )));
        }
        self.writer.write_all(&record)?;
        self.writer.write_all(b"\n")?;
        self.writer.flush()?;
        Ok(())
    }

    /// Returns the wrapped writer after all desired records have been emitted.
    pub fn into_inner(self) -> W {
        self.writer
    }
}
