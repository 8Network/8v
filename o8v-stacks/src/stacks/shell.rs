//! Shell stack — shellcheck for shell script linting.
//!
//! Finds all .sh files in the project and runs shellcheck on them.
//! If no shell files are found, the check passes (nothing to check).

use crate::enrich::enrich;
use crate::runner::run_tool;
use crate::tool_resolution::scan_project;
use o8v_core::{Check, CheckContext, CheckOutcome};

use std::process::Command;

/// Returns all checks for the shell stack.
pub fn checks() -> Vec<Box<dyn Check>> {
    vec![Box::new(ShellCheckCheck)]
}

struct ShellCheckCheck;

impl Check for ShellCheckCheck {
    fn name(&self) -> &'static str {
        "shellcheck"
    }

    fn run(&self, project_dir: &o8v_fs::ContainmentRoot, ctx: &CheckContext) -> CheckOutcome {
        // Find all .sh files in the project
        let shell_files = find_shell_files(project_dir);

        // If no shell files found, pass (nothing to check)
        if shell_files.is_empty() {
            tracing::debug!("no .sh files found, skipping shellcheck");
            return CheckOutcome::passed(
                String::new(),
                String::new(),
                o8v_core::diagnostic::ParseStatus::Parsed,
                false,
                false,
            );
        }

        // Run shellcheck on all found files
        let mut cmd = Command::new("shellcheck");
        cmd.arg("-f").arg("json");
        cmd.args(&shell_files).current_dir(project_dir.as_path());

        let outcome = run_tool(cmd, "shellcheck", ctx.timeout, ctx.interrupted);
        enrich(
            outcome,
            project_dir,
            "shellcheck",
            "shell",
            crate::parse::shellcheck::parse,
        )
    }
}

/// Find all .sh files in a project directory using o8v-fs.
fn find_shell_files(project_dir: &o8v_fs::ContainmentRoot) -> Vec<String> {
    let Some(scan) = scan_project(project_dir) else {
        return vec![];
    };
    scan.entries_with_extension("sh")
        .map(|e| e.name.clone())
        .collect()
}
