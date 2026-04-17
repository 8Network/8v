// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! The 8v CLI — entry point. Parses input, forwards to dispatch. No logic here.

pub(crate) mod cli;
pub(crate) mod commands;
mod hooks;
mod init;
mod mcp;
pub(crate) mod util;

mod signal;
mod tracing;

use std::process::ExitCode;
use std::sync::atomic::AtomicBool;

fn main() -> ExitCode {
    tracing::init();

    let interrupted: &'static AtomicBool = Box::leak(Box::new(AtomicBool::new(false)));
    signal::install(interrupted);

    use clap::Parser;
    let cli = cli::Cli::parse();

    match cli.command {
        commands::Command::Init(args) => init::run(&args),
        commands::Command::Mcp => {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            match rt.block_on(mcp::serve()) {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("error: MCP server failed: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        command => {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            match rt.block_on(commands::dispatch_command(
                command,
                o8v_core::caller::Caller::Cli,
                interrupted,
            )) {
                Ok((output, exit_code, use_stderr)) => {
                    use std::io::Write;
                    let result = if use_stderr {
                        std::io::stderr().write_all(output.as_bytes())
                    } else {
                        std::io::stdout().write_all(output.as_bytes())
                    };
                    if let Err(e) = result {
                        if e.kind() != std::io::ErrorKind::BrokenPipe {
                            eprintln!("error: write failed: {e}");
                            return ExitCode::FAILURE;
                        }
                    }
                    exit_code
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    ExitCode::FAILURE
                }
            }
        }
    }
}
