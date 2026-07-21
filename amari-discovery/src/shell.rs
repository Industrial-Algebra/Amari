// SPDX-License-Identifier: MIT OR Apache-2.0

//! Human shell input handling over the shared CLI command dispatcher.

use std::{
    ffi::OsString,
    io::{BufRead, Read, Write},
};

use serde::Deserialize;
use serde_json::{Map, Value};

use crate::{DiscoveryError, DiscoveryResult, SCHEMA_V1};

const MAX_SHELL_INPUT_BYTES: u64 = 64 * 1024;

pub(crate) fn run_human<R, W, F>(
    reader: &mut R,
    writer: &mut W,
    mut dispatch: F,
) -> DiscoveryResult<()>
where
    R: BufRead,
    W: Write,
    F: FnMut(Vec<OsString>, &mut W) -> DiscoveryResult<()>,
{
    writeln!(
        writer,
        "Amari discovery shell. Type `help` for commands or `exit` to leave."
    )?;
    loop {
        write!(writer, "amari> ")?;
        writer.flush()?;

        let mut bytes = Vec::new();
        if read_bounded_line(reader, &mut bytes)? == 0 {
            break;
        }
        let line = std::str::from_utf8(&bytes).map_err(|_| {
            DiscoveryError::InvalidInput("shell input must be valid UTF-8".to_owned())
        })?;
        let arguments = tokenize(line.trim_end_matches(['\r', '\n']))?;
        if arguments.is_empty() {
            continue;
        }
        match arguments[0].as_str() {
            "exit" | "quit" if arguments.len() == 1 => break,
            "help" if arguments.len() == 1 => write_help(writer)?,
            _ => dispatch(arguments.into_iter().map(OsString::from).collect(), writer)?,
        }
    }
    Ok(())
}

pub(crate) fn run_json<R, W, F>(
    reader: &mut R,
    writer: &mut W,
    mut dispatch: F,
) -> DiscoveryResult<()>
where
    R: Read,
    W: Write,
    F: FnMut(Vec<OsString>, &mut W) -> DiscoveryResult<()>,
{
    let mut bytes = Vec::new();
    Read::by_ref(reader)
        .take(MAX_SHELL_INPUT_BYTES + 1)
        .read_to_end(&mut bytes)?;
    ensure_bounded_input(&bytes)?;
    if bytes.iter().all(u8::is_ascii_whitespace) {
        return Err(DiscoveryError::InvalidInput(
            "shell JSON mode requires exactly one request".to_owned(),
        ));
    }
    dispatch(parse_machine_request(&bytes)?, writer)
}

pub(crate) fn run_ndjson<R, W, F>(
    reader: &mut R,
    writer: &mut W,
    mut dispatch: F,
) -> DiscoveryResult<()>
where
    R: BufRead,
    W: Write,
    F: FnMut(Vec<OsString>, &mut W) -> DiscoveryResult<()>,
{
    loop {
        let mut bytes = Vec::new();
        if read_bounded_line(reader, &mut bytes)? == 0 {
            return Ok(());
        }
        if bytes.iter().all(u8::is_ascii_whitespace) {
            return Err(DiscoveryError::InvalidInput(
                "shell NDJSON requests must not be blank".to_owned(),
            ));
        }
        dispatch(parse_machine_request(&bytes)?, writer)?;
    }
}

fn read_bounded_line(reader: &mut impl BufRead, bytes: &mut Vec<u8>) -> DiscoveryResult<usize> {
    let count = Read::by_ref(reader)
        .take(MAX_SHELL_INPUT_BYTES + 1)
        .read_until(b'\n', bytes)?;
    ensure_bounded_input(bytes)?;
    Ok(count)
}

fn ensure_bounded_input(bytes: &[u8]) -> DiscoveryResult<()> {
    if bytes.len() as u64 > MAX_SHELL_INPUT_BYTES {
        return Err(DiscoveryError::LimitExceeded(
            "shell input exceeds 65536 bytes".to_owned(),
        ));
    }
    Ok(())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MachineRequest {
    schema_version: String,
    command: String,
    arguments: Map<String, Value>,
}

fn parse_machine_request(bytes: &[u8]) -> DiscoveryResult<Vec<OsString>> {
    let request: MachineRequest = serde_json::from_slice(bytes)
        .map_err(|error| DiscoveryError::InvalidInput(format!("invalid shell request: {error}")))?;
    if request.schema_version != SCHEMA_V1 {
        return Err(DiscoveryError::InvalidInput(format!(
            "unsupported shell request schema `{}`",
            request.schema_version
        )));
    }
    if request.arguments.len() > 32 {
        return Err(DiscoveryError::LimitExceeded(format!(
            "shell request arguments {} exceed limit 32",
            request.arguments.len()
        )));
    }
    machine_arguments(&request.command, request.arguments)
}

fn machine_arguments(
    command: &str,
    mut arguments: Map<String, Value>,
) -> DiscoveryResult<Vec<OsString>> {
    let mut result: Vec<OsString> = command.split('.').map(OsString::from).collect();
    match command {
        "capabilities" | "probe.list" => {}
        "discover.search" => push_required(&mut result, &mut arguments, "query", None)?,
        "discover.detail" | "discover.graph" | "discover.example" => {
            push_required(&mut result, &mut arguments, "identifier", None)?;
        }
        "inspect" => push_optional(&mut result, &mut arguments, "path", None)?,
        "recommend" => {
            push_optional(&mut result, &mut arguments, "path", None)?;
            push_optional(&mut result, &mut arguments, "goal", Some("--goal"))?;
            push_optional(
                &mut result,
                &mut arguments,
                "goal_file",
                Some("--goal-file"),
            )?;
            push_optional(
                &mut result,
                &mut arguments,
                "probe_results",
                Some("--probe-results"),
            )?;
        }
        "plan" => {
            push_required(&mut result, &mut arguments, "candidate_id", None)?;
            push_required(
                &mut result,
                &mut arguments,
                "recommendation",
                Some("--recommendation"),
            )?;
            push_optional(&mut result, &mut arguments, "project", Some("--project"))?;
        }
        "probe.describe" => {
            push_required(&mut result, &mut arguments, "probe_id", None)?;
        }
        "probe.run" => {
            push_required(&mut result, &mut arguments, "probe_id", None)?;
            push_optional(&mut result, &mut arguments, "input", Some("--input"))?;
            push_optional(&mut result, &mut arguments, "plan", Some("--plan"))?;
            if take_optional_bool(&mut arguments, "dry_run")?.unwrap_or(false) {
                result.push(OsString::from("--dry-run"));
            }
        }
        "schema" => push_optional(&mut result, &mut arguments, "kind", None)?,
        _ => {
            return Err(DiscoveryError::InvalidInput(format!(
                "unknown shell command `{command}`"
            )));
        }
    }
    if let Some(name) = arguments.keys().next() {
        return Err(DiscoveryError::InvalidInput(format!(
            "unknown argument `{name}` for shell command `{command}`"
        )));
    }
    Ok(result)
}

fn push_required(
    result: &mut Vec<OsString>,
    arguments: &mut Map<String, Value>,
    name: &str,
    flag: Option<&str>,
) -> DiscoveryResult<()> {
    let value = take_string(arguments, name)?.ok_or_else(|| {
        DiscoveryError::InvalidInput(format!("shell request requires argument `{name}`"))
    })?;
    if let Some(flag) = flag {
        result.push(OsString::from(flag));
    }
    result.push(OsString::from(value));
    Ok(())
}

fn push_optional(
    result: &mut Vec<OsString>,
    arguments: &mut Map<String, Value>,
    name: &str,
    flag: Option<&str>,
) -> DiscoveryResult<()> {
    if let Some(value) = take_string(arguments, name)? {
        if let Some(flag) = flag {
            result.push(OsString::from(flag));
        }
        result.push(OsString::from(value));
    }
    Ok(())
}

fn take_string(arguments: &mut Map<String, Value>, name: &str) -> DiscoveryResult<Option<String>> {
    arguments
        .remove(name)
        .map(|value| {
            value.as_str().map(str::to_owned).ok_or_else(|| {
                DiscoveryError::InvalidInput(format!(
                    "shell request argument `{name}` must be a string"
                ))
            })
        })
        .transpose()
}

fn take_optional_bool(
    arguments: &mut Map<String, Value>,
    name: &str,
) -> DiscoveryResult<Option<bool>> {
    arguments
        .remove(name)
        .map(|value| {
            value.as_bool().ok_or_else(|| {
                DiscoveryError::InvalidInput(format!(
                    "shell request argument `{name}` must be a boolean"
                ))
            })
        })
        .transpose()
}

fn write_help(writer: &mut impl Write) -> DiscoveryResult<()> {
    writeln!(writer, "Shell commands:")?;
    writeln!(writer, "  capabilities")?;
    writeln!(writer, "  discover search|detail|graph|example ...")?;
    writeln!(writer, "  inspect [PATH]")?;
    writeln!(writer, "  recommend [PATH] --goal TEXT|--goal-file FILE")?;
    writeln!(
        writer,
        "  plan CANDIDATE --recommendation FILE [--project PATH]"
    )?;
    writeln!(writer, "  probe list|describe|run ...")?;
    writeln!(writer, "  schema [request|response|goal|plan|probe]")?;
    writeln!(writer, "  help")?;
    writeln!(writer, "  exit")?;
    Ok(())
}

fn tokenize(line: &str) -> DiscoveryResult<Vec<String>> {
    let mut tokens = Vec::new();
    let mut token = String::new();
    let mut quote = None;
    let mut escaped = false;
    let mut active = false;

    for character in line.chars() {
        if escaped {
            token.push(character);
            escaped = false;
            active = true;
            continue;
        }
        match (quote, character) {
            (_, '\\') => {
                escaped = true;
                active = true;
            }
            (Some(expected), current) if current == expected => {
                quote = None;
                active = true;
            }
            (None, '\'' | '"') => {
                quote = Some(character);
                active = true;
            }
            (None, current) if current.is_whitespace() => {
                if active {
                    tokens.push(std::mem::take(&mut token));
                    active = false;
                }
            }
            (_, current) => {
                token.push(current);
                active = true;
            }
        }
    }
    if escaped {
        return Err(DiscoveryError::InvalidInput(
            "shell input ends with an incomplete escape".to_owned(),
        ));
    }
    if quote.is_some() {
        return Err(DiscoveryError::InvalidInput(
            "shell input contains an unterminated quote".to_owned(),
        ));
    }
    if active {
        tokens.push(token);
    }
    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::tokenize;

    #[test]
    fn tokenizer_preserves_quoted_and_escaped_arguments() {
        assert_eq!(
            tokenize("recommend --goal \"geometric product\" path\\ with\\ spaces").unwrap(),
            [
                "recommend",
                "--goal",
                "geometric product",
                "path with spaces"
            ]
        );
    }

    #[test]
    fn tokenizer_rejects_incomplete_input() {
        assert!(tokenize("recommend --goal 'missing").is_err());
        assert!(tokenize("inspect missing\\").is_err());
    }
}
