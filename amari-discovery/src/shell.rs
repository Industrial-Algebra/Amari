// SPDX-License-Identifier: MIT OR Apache-2.0

//! Human shell input handling over the shared CLI command dispatcher.

use std::{
    ffi::OsString,
    io::{BufRead, Read, Write},
};

use crate::{DiscoveryError, DiscoveryResult};

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
        let count = Read::by_ref(reader)
            .take(MAX_SHELL_INPUT_BYTES + 1)
            .read_until(b'\n', &mut bytes)?;
        if count == 0 {
            break;
        }
        if count as u64 > MAX_SHELL_INPUT_BYTES {
            return Err(DiscoveryError::LimitExceeded(
                "shell input exceeds 65536 bytes".to_owned(),
            ));
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
