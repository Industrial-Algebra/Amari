// SPDX-License-Identifier: MIT OR Apache-2.0

use std::process::ExitCode;

fn main() -> ExitCode {
    match amari_discovery::cli::run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            amari_discovery::cli::report_error(&error);
            ExitCode::from(error.exit_code())
        }
    }
}
