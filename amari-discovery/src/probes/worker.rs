// SPDX-License-Identifier: MIT OR Apache-2.0

//! Private bounded framing and child dispatch for the fixed probe worker.

use std::io::{self, Read, Write};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{ProbeEngine, ProbeEngineLimits, ProbeExecution};
use crate::{DiscoveryError, DiscoveryResult, ProbeId, Provenance};

const FRAME_HEADER_BYTES: usize = 4;
const MAX_FRAME_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct WorkerRequest {
    probe_id: ProbeId,
    input: Value,
    limits: ProbeEngineLimits,
    provenance: Provenance,
}

#[derive(Debug, Deserialize, Serialize)]
pub(super) struct WorkerResponse {
    pub(super) provenance: Provenance,
    pub(super) execution: ProbeExecution,
}

pub(crate) fn run_stdio() -> DiscoveryResult<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let request = read_request(&mut stdin.lock())?;
    let execution =
        ProbeEngine::with_limits(request.limits)?.execute(&request.probe_id, &request.input)?;
    let response = WorkerResponse {
        provenance: request.provenance,
        execution,
    };
    write_response(&mut stdout.lock(), &response)
}

fn read_request(reader: &mut impl Read) -> DiscoveryResult<WorkerRequest> {
    let body = read_frame(reader)?;
    reject_trailing_bytes(reader)?;
    Ok(serde_json::from_slice(&body)?)
}

fn write_response(writer: &mut impl Write, response: &WorkerResponse) -> DiscoveryResult<()> {
    let body = serde_json::to_vec(response)?;
    write_frame(writer, &body)
}

// Task 21C consumes supervised worker responses through this decoder.
#[allow(dead_code)]
pub(super) fn decode_response_frame(bytes: &[u8]) -> DiscoveryResult<WorkerResponse> {
    let mut reader = io::Cursor::new(bytes);
    let body = read_frame(&mut reader)?;
    reject_trailing_bytes(&mut reader)?;
    Ok(serde_json::from_slice(&body)?)
}

fn read_frame(reader: &mut impl Read) -> DiscoveryResult<Vec<u8>> {
    let mut header = [0_u8; FRAME_HEADER_BYTES];
    read_exact_protocol(
        reader,
        &mut header,
        "probe worker frame header is truncated",
    )?;
    let length = usize::try_from(u32::from_be_bytes(header)).map_err(|_| {
        DiscoveryError::LimitExceeded("probe worker frame length does not fit usize".to_owned())
    })?;
    if length == 0 {
        return Err(DiscoveryError::InvalidInput(
            "probe worker frame body must not be empty".to_owned(),
        ));
    }
    if length > MAX_FRAME_BYTES {
        return Err(DiscoveryError::LimitExceeded(format!(
            "probe worker frame bytes {length} exceeds limit {MAX_FRAME_BYTES}"
        )));
    }

    let mut body = vec![0_u8; length];
    read_exact_protocol(reader, &mut body, "probe worker frame body is truncated")?;
    Ok(body)
}

fn write_frame(writer: &mut impl Write, body: &[u8]) -> DiscoveryResult<()> {
    if body.is_empty() {
        return Err(DiscoveryError::InvalidInput(
            "probe worker response frame must not be empty".to_owned(),
        ));
    }
    if body.len() > MAX_FRAME_BYTES {
        return Err(DiscoveryError::LimitExceeded(format!(
            "probe worker response bytes {} exceeds limit {MAX_FRAME_BYTES}",
            body.len()
        )));
    }
    let length = u32::try_from(body.len()).map_err(|_| {
        DiscoveryError::LimitExceeded("probe worker response length exceeds u32".to_owned())
    })?;
    writer.write_all(&length.to_be_bytes())?;
    writer.write_all(body)?;
    writer.flush()?;
    Ok(())
}

fn reject_trailing_bytes(reader: &mut impl Read) -> DiscoveryResult<()> {
    let mut trailing = [0_u8; 1];
    match reader.read(&mut trailing) {
        Ok(0) => Ok(()),
        Ok(_) => Err(DiscoveryError::InvalidInput(
            "probe worker accepts exactly one request frame".to_owned(),
        )),
        Err(error) => Err(DiscoveryError::Io(error)),
    }
}

fn read_exact_protocol(
    reader: &mut impl Read,
    buffer: &mut [u8],
    truncated_message: &str,
) -> DiscoveryResult<()> {
    match reader.read_exact(buffer) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => {
            Err(DiscoveryError::InvalidInput(truncated_message.to_owned()))
        }
        Err(error) => Err(DiscoveryError::Io(error)),
    }
}
