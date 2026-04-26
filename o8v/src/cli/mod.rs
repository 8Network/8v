// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! CLI interface — parses arguments, forwards to commands.

pub(crate) mod common;
pub(crate) mod time_utc;
pub mod version;

use clap::Parser;

#[derive(Parser)]
#[command(
    name = "8v",
    version = version::short(),
    about = "Code reliability tool — one command checks everything"
)]
pub struct Cli {
    /// Print full build provenance (commit, branch, rustc, binary path, …).
    #[arg(long = "build-info", action = clap::ArgAction::SetTrue)]
    pub build_info: bool,

    #[command(subcommand)]
    pub command: Option<crate::commands::Command>,
}
