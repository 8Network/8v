// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! CLI interface — parses arguments, forwards to commands.

pub(crate) mod common;
pub(crate) mod time_utc;
pub(crate) mod version;

use clap::Parser;

#[derive(Parser)]
#[command(
    name = "8v",
    version,
    long_version = version::long(),
    about = "Code reliability tool — one command checks everything"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: crate::commands::Command,
}
