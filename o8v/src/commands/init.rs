// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! InitCommand — implements Command trait for `8v init`.

use o8v_core::command::{Command, CommandContext, CommandError};
use o8v_core::render::init_report::InitReport;
use std::process::ExitCode;

pub struct InitCommand {
    pub args: crate::init::Args,
}

impl Command for InitCommand {
    type Report = InitReport;

    async fn execute(&self, _ctx: &CommandContext) -> Result<Self::Report, CommandError> {
        let exit = crate::init::run(&self.args);
        let success = exit == ExitCode::SUCCESS;
        Ok(InitReport { success })
    }
}
