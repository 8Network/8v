// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! The 8v CLI — entry point. Parses input, forwards to dispatch. No logic here.

use o8v::{cli, commands, init, mcp, signal, tracing};

use std::process::ExitCode;
use std::sync::atomic::AtomicBool;

fn main() -> ExitCode {
    tracing::init();

    let interrupted: &'static AtomicBool = Box::leak(Box::new(AtomicBool::new(false)));
    signal::install(interrupted);

    // Capture the raw CLI argv tail BEFORE clap consumes it for structured
    // parsing. This is what lands in events.ndjson so agent behavior (which
    // flags, which paths) is reconstructable from the event log.
    let argv: Vec<String> = std::env::args().skip(1).collect();

    // Resolve the default audience ONCE at process entry.
    // _8V_AGENT=1 (or any non-empty value) signals that this CLI invocation
    // is inside an AI agent; default to Agent so all commands behave
    // consistently without requiring --plain on every call.
    let cli_default_audience = match std::env::var("_8V_AGENT") {
        Ok(v) if !v.is_empty() => o8v_core::render::Audience::Agent,
        _ => o8v_core::render::Audience::Human,
    };

    use clap::Parser;
    let cli = cli::Cli::parse();

    match cli.command {
        commands::Command::Init(args) => init::run(&args),
        commands::Command::Mcp => {
            let rt = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(e) => {
                    eprintln!("error: failed to initialize async runtime: {e} — check OS thread limits or sandbox restrictions");
                    return ExitCode::FAILURE;
                }
            };
            match rt.block_on(mcp::serve()) {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("error: MCP server failed: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        command => {
            let rt = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(e) => {
                    eprintln!("error: failed to initialize async runtime: {e} — check OS thread limits or sandbox restrictions");
                    return ExitCode::FAILURE;
                }
            };
            match rt.block_on(commands::dispatch_command(
                command,
                o8v_core::caller::Caller::Cli,
                argv,
                interrupted,
                cli_default_audience,
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
