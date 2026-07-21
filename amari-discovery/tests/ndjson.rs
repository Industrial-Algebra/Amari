// SPDX-License-Identifier: MIT OR Apache-2.0

//! Shared bounded NDJSON record framing contract.

use std::io::{self, Write};

use amari_discovery::{
    CatalogIdentity, Compatibility, DiscoveryError, Envelope, NdjsonWriter, ReplayMetadata,
    DEFAULT_MAX_NDJSON_RECORD_BYTES,
};
use serde_json::{json, Value};

#[derive(Default)]
struct ObservedWriter {
    bytes: Vec<u8>,
    flushes: usize,
    fail_write: bool,
    fail_flush: bool,
}

impl Write for ObservedWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if self.fail_write {
            return Err(io::Error::new(io::ErrorKind::BrokenPipe, "fixture write"));
        }
        self.bytes.extend_from_slice(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.flushes += 1;
        if self.fail_flush {
            return Err(io::Error::new(io::ErrorKind::BrokenPipe, "fixture flush"));
        }
        Ok(())
    }
}

fn envelope(data: Value) -> Envelope<Value> {
    let mut envelope = Envelope::new(
        data,
        CatalogIdentity {
            version: "fixture".to_owned(),
            hash: "fixture-catalog".to_owned(),
        },
        Compatibility {
            status: "compatible".to_owned(),
            reasons: Vec::new(),
        },
        ReplayMetadata {
            replayable: true,
            required_hashes: vec!["catalog_hash".to_owned()],
            reasons: Vec::new(),
        },
    );
    envelope.provenance.input_hash = Some("fixture-input".to_owned());
    envelope
}

#[test]
fn every_write_emits_one_complete_object_line_and_flushes() {
    let mut output = ObservedWriter::default();
    {
        let mut writer = NdjsonWriter::new(&mut output).unwrap();
        writer.write(&json!({"sequence": 1})).unwrap();
        writer.write(&json!({"sequence": 2})).unwrap();
    }

    assert_eq!(output.flushes, 2);
    let lines = output
        .bytes
        .split(|byte| *byte == b'\n')
        .collect::<Vec<_>>();
    assert_eq!(lines.len(), 3);
    assert!(lines[2].is_empty());
    assert_eq!(
        serde_json::from_slice::<Value>(lines[0]).unwrap(),
        json!({"sequence": 1})
    );
    assert_eq!(
        serde_json::from_slice::<Value>(lines[1]).unwrap(),
        json!({"sequence": 2})
    );
}

#[test]
fn embedded_newlines_are_escaped_without_splitting_records() {
    let mut bytes = Vec::new();
    NdjsonWriter::new(&mut bytes)
        .unwrap()
        .write(&json!({"message": "first\nsecond\r\nthird"}))
        .unwrap();

    assert_eq!(bytes.iter().filter(|byte| **byte == b'\n').count(), 1);
    assert!(String::from_utf8(bytes.clone())
        .unwrap()
        .contains("first\\nsecond\\r\\nthird"));
    assert_eq!(
        serde_json::from_slice::<Value>(&bytes[..bytes.len() - 1]).unwrap()["message"],
        "first\nsecond\r\nthird"
    );
}

#[test]
fn record_limit_is_checked_before_any_bytes_are_written() {
    let value = json!({"payload": "bounded"});
    let encoded = serde_json::to_vec(&value).unwrap();
    let mut exact = Vec::new();
    NdjsonWriter::with_max_record_bytes(&mut exact, encoded.len())
        .unwrap()
        .write(&value)
        .unwrap();
    assert_eq!(exact.len(), encoded.len() + 1);

    let mut rejected = Vec::new();
    let error = NdjsonWriter::with_max_record_bytes(&mut rejected, encoded.len() - 1)
        .unwrap()
        .write(&value)
        .unwrap_err();
    assert!(matches!(
        error,
        DiscoveryError::LimitExceeded(message) if message.contains("NDJSON record bytes")
    ));
    assert!(rejected.is_empty());
    assert_eq!(DEFAULT_MAX_NDJSON_RECORD_BYTES, 1024 * 1024);
}

#[test]
fn zero_limit_and_write_or_flush_failures_are_typed() {
    assert!(matches!(
        NdjsonWriter::with_max_record_bytes(Vec::new(), 0),
        Err(DiscoveryError::InvalidInput(message)) if message.contains("greater than zero")
    ));

    let mut write_failure = ObservedWriter {
        fail_write: true,
        ..ObservedWriter::default()
    };
    let error = NdjsonWriter::new(&mut write_failure)
        .unwrap()
        .write(&json!({"ok": true}))
        .unwrap_err();
    assert!(matches!(error, DiscoveryError::Io(_)));

    let mut flush_failure = ObservedWriter {
        fail_flush: true,
        ..ObservedWriter::default()
    };
    let error = NdjsonWriter::new(&mut flush_failure)
        .unwrap()
        .write(&json!({"ok": true}))
        .unwrap_err();
    assert!(matches!(error, DiscoveryError::Io(_)));
    assert_eq!(flush_failure.flushes, 1);
}

#[test]
fn typed_envelope_provenance_survives_ndjson_framing() {
    let expected = envelope(json!({"result": "ok"}));
    let mut bytes = Vec::new();
    NdjsonWriter::new(&mut bytes)
        .unwrap()
        .write(&expected)
        .unwrap();
    let parsed: Envelope<Value> = serde_json::from_slice(&bytes[..bytes.len() - 1]).unwrap();

    assert_eq!(parsed, expected);
    assert_eq!(parsed.provenance.catalog.hash, "fixture-catalog");
    assert_eq!(
        parsed.provenance.input_hash.as_deref(),
        Some("fixture-input")
    );
}
