//! .NET stack — dotnet build (with --warnaserror).
//!
//! Passes the explicit project/solution file to `dotnet build` using the same
//! priority as detection (.slnx > .sln > .csproj). Without it, `dotnet build`
//! fails with MSB1011 when multiple files coexist (e.g. App.slnx + App.sln).

use crate::enrich::enrich;
use crate::runner::run_tool;
use crate::stack_tools::{BuildTool, FormatTool, StackTools, TestTool};
use crate::tool_resolution::scan_project;
use o8v_core::{Check, CheckContext, CheckOutcome};

use std::process::Command;

/// Returns all tools for the dotnet stack.
pub fn tools() -> StackTools {
    StackTools {
        checks: vec![Box::new(DotnetCheck)],
        formatter: Some(FormatTool {
            program: "dotnet",
            format_args: &["format"],
            check_args: &["format", "--verify-no-changes"],
            check_dirty_on_stdout: false,
            needs_node_resolution: false,
        }),
        test_runner: Some(TestTool {
            program: "dotnet",
            args: &["test"],
        }),
        build_tool: Some(BuildTool {
            program: "dotnet",
            args: &["build"],
        }),
        error_extractor: None,
    }
}

struct DotnetCheck;

impl Check for DotnetCheck {
    fn name(&self) -> &'static str {
        "dotnet build"
    }

    fn run(&self, project_dir: &o8v_fs::ContainmentRoot, ctx: &CheckContext) -> CheckOutcome {
        // Discover the target file with the same priority as detection:
        // .slnx > .sln > .csproj. Without this, `dotnet build` fails with
        // MSB1011 when multiple project/solution files coexist.
        let target = find_dotnet_target(project_dir);

        let mut cmd = Command::new("dotnet");
        cmd.arg("build");
        if let Some(ref file) = target {
            cmd.arg(file);
        }
        cmd.args(["--warnaserror", "--tl:off", "/clp:NoSummary"])
            .current_dir(project_dir.as_path());

        let outcome = run_tool(cmd, "dotnet build", ctx.timeout, ctx.interrupted);
        enrich(
            outcome,
            project_dir,
            "dotnet build",
            "dotnet",
            crate::parse::dotnet::parse,
        )
    }
}

/// Find the highest-priority .NET target file in a directory using o8v-fs.
/// Priority: .slnx > .sln > .csproj (matches detection in o8v-project).
fn find_dotnet_target(project_dir: &o8v_fs::ContainmentRoot) -> Option<String> {
    let scan = scan_project(project_dir)?;
    // Same priority as o8v-project detection: .slnx > .sln > .csproj
    for ext in &["slnx", "sln", "csproj"] {
        if let Some(entry) = scan.entries_with_extension(ext).next() {
            return Some(entry.name.clone());
        }
    }
    None
}
