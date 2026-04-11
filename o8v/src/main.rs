// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! The 8v CLI — code reliability tool.
//!
//! One command checks everything. Detects projects, runs language checks,
//! captures output, formats results. Supports human, plain, and JSON output.

pub(crate) mod cli;
pub(crate) mod commands;
mod events;
mod hooks;
mod init;
mod mcp;
pub(crate) mod util;

use std::process::ExitCode;
use std::sync::atomic::AtomicBool;

fn main() -> ExitCode {
    cli::init_tracing();

    // Leak the Arc to get &'static AtomicBool. This is intentional:
    // the flag lives for the entire process, and &'static lets it flow
    // through CheckContext → run_tool → ProcessConfig without lifetime issues.
    let interrupted: &'static AtomicBool = Box::leak(Box::new(AtomicBool::new(false)));
    cli::signal::install_signal_handler(interrupted);

    use clap::Parser;
    let cli = cli::Cli::parse();

    match cli.command {
        cli::Command::Build(args) => {
            let audience = args.audience();
            let cmd = commands::build::BuildCommand { args };
            match cli::cli_dispatch(&cmd, audience, interrupted) {
                Ok((output, report)) => {
                    print!("{output}");
                    cli::process_exit_code(&report.process)
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        cli::Command::Check(args) => {
            let audience = args.audience();
            let cmd = commands::check::CheckCommand { args };
            match cli::cli_dispatch(&cmd, audience, interrupted) {
                Ok((output, report)) => {
                    // Human output goes to stderr; machine/agent output goes to stdout.
                    if audience == o8v_core::render::Audience::Human {
                        eprint!("{output}");
                    } else {
                        use std::io::Write;
                        if let Err(e) = std::io::stdout().write_all(output.as_bytes()) {
                            if e.kind() != std::io::ErrorKind::BrokenPipe {
                                eprintln!("error: write failed: {e}");
                                return ExitCode::FAILURE;
                            }
                            // Broken pipe — consumer stopped reading. Not a check failure.
                            return if report.results().is_empty()
                                && report.detection_errors().is_empty()
                            {
                                ExitCode::from(cli::common::EXIT_NOTHING)
                            } else {
                                ExitCode::SUCCESS
                            };
                        }
                    }
                    if report.results().is_empty() && report.detection_errors().is_empty() {
                        ExitCode::from(cli::common::EXIT_NOTHING)
                    } else if report.is_ok() {
                        ExitCode::SUCCESS
                    } else {
                        ExitCode::FAILURE
                    }
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        cli::Command::Fmt(args) => {
            let audience = args.audience();
            let cmd = commands::fmt::FmtCommand { args };
            match cli::cli_dispatch(&cmd, audience, interrupted) {
                Ok((output, report)) => {
                    // Human output goes to stderr; machine/agent output goes to stdout.
                    if audience == o8v_core::render::Audience::Human {
                        eprint!("{output}");
                    } else {
                        print!("{output}");
                    }
                    if report.entries.is_empty() && report.detection_errors.is_empty() {
                        ExitCode::from(cli::common::EXIT_NOTHING)
                    } else if report.is_ok() {
                        ExitCode::SUCCESS
                    } else {
                        ExitCode::FAILURE
                    }
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        cli::Command::Test(args) => {
            let audience = args.audience();
            let cmd = commands::test::TestCommand { args };
            match cli::cli_dispatch(&cmd, audience, interrupted) {
                Ok((output, report)) => {
                    print!("{output}");
                    cli::process_exit_code(&report.process)
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        cli::Command::Init(args) => init::run(&args),
        cli::Command::Hooks(args) => {
            let cmd = commands::hooks::HooksCommand { args };
            match cli::cli_dispatch(&cmd, o8v_core::render::Audience::Human, interrupted) {
                Ok((output, report)) => {
                    eprint!("{output}");
                    if report.success {
                        ExitCode::SUCCESS
                    } else {
                        ExitCode::from(report.exit_code)
                    }
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        cli::Command::Upgrade(args) => {
            let cmd = commands::upgrade::UpgradeCommand { args };
            cli::cli_run(&cmd, o8v_core::render::Audience::Human, interrupted)
        }
        cli::Command::Read(args) => {
            let audience = args.audience();
            let cmd = commands::read::ReadCommand { args };
            cli::cli_run(&cmd, audience, interrupted)
        }
        cli::Command::Write(args) => {
            let audience = args.audience();
            let cmd = commands::write::WriteCommand { args };
            cli::cli_run(&cmd, audience, interrupted)
        }
        cli::Command::Run(args) => {
            let audience = args.audience();
            let cmd = commands::run::RunCommand { args };
            match cli::cli_dispatch(&cmd, audience, interrupted) {
                Ok((output, report)) => {
                    print!("{output}");
                    cli::process_exit_code(&report.process)
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        cli::Command::Search(args) => {
            let audience = args.audience();
            let cmd = commands::search::SearchCommand { args };
            cli::cli_run(&cmd, audience, interrupted)
        }
        cli::Command::Ls(args) => {
            let audience = args.audience();
            let cmd = commands::ls::LsCommand { args };
            cli::cli_run(&cmd, audience, interrupted)
        }
        cli::Command::Mcp => {
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
    }
}
