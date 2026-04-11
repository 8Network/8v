// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! HooksCommand — implements Command trait for `8v hooks`.

use o8v_core::command::{Command, CommandContext, CommandError};
use o8v_core::event_channel::EventChannel;
use o8v_core::render::hooks_report::HooksReport;

pub struct HooksCommand {
    pub args: crate::hooks::Args,
}

impl Command for HooksCommand {
    type Report = HooksReport;
    type Event = ();

    async fn execute(
        &self,
        ctx: &CommandContext,
        _events: EventChannel<Self::Event>,
    ) -> Result<Self::Report, CommandError> {
        let exit_code = crate::hooks::run_code(&self.args, ctx.interrupted);
        let success = exit_code == 0;

        Ok(HooksReport {
            hooks: vec![],
            exit_code,
            success,
        })
    }
}
