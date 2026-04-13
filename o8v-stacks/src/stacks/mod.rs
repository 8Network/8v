//! Per-stack check definitions.
//!
//! Each stack knows HOW to check: which tools, which flags, max strictness.
//! Adding a stack means adding a file.

mod deno;
mod dockerfile;
mod dotnet;
mod erlang;
mod go;
mod helm;
mod java;
mod javascript;
mod kotlin;
mod kustomize;
mod node;
mod python;
mod ruby;
mod rust;
mod shell;
mod swift;
mod terraform;
mod typescript;

use crate::stack_tools::StackTools;
use o8v_core::Check;
use o8v_core::project::Stack;

/// Returns all tools for a given stack.
///
/// This is the single source of truth for a stack's capabilities:
/// checks, formatter, and test runner.
#[must_use]
pub fn tools_for(stack: Stack) -> StackTools {
    let mut tools = match stack {
        Stack::Rust => rust::tools(),
        Stack::TypeScript => typescript::tools(),
        Stack::JavaScript => javascript::tools(),
        Stack::Python => python::tools(),
        Stack::Go => go::tools(),
        Stack::Deno => deno::tools(),
        Stack::DotNet => dotnet::tools(),
        Stack::Ruby => ruby::tools(),
        Stack::Java => java::tools(),
        Stack::Kotlin => kotlin::tools(),
        Stack::Swift => swift::tools(),
        Stack::Terraform => terraform::tools(),
        Stack::Dockerfile => dockerfile::tools(),
        Stack::Helm => helm::tools(),
        Stack::Kustomize => kustomize::tools(),
        Stack::Erlang => erlang::tools(),
        _ => {
            tracing::warn!(%stack, "no tools defined for stack");
            StackTools {
                checks: vec![],
                formatter: None,
                test_runner: None,
                build_tool: None,
            }
        }
    };

    // Add cross-cutting checks that apply to all stacks
    tools.checks.extend(shell::checks());

    tools
}

/// Returns all checks for a given stack (backward compatible).
///
/// Checks no longer carry timeout — that's in `CheckContext`, passed at run time.
#[must_use]
pub fn checks_for(stack: Stack) -> Vec<Box<dyn Check>> {
    tools_for(stack).checks
}
