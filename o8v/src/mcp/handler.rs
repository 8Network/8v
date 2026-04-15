// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! MCP interface — parses command string, forwards to dispatch. No logic here.

use rmcp::{Peer, RoleServer};

/// Parse and execute an 8v command.
///
/// Returns `Ok(text)` on success, `Err(text)` on failure. The MCP tool macro
/// converts `Err` into a `CallToolResult` with `is_error: true`, so the agent
/// can distinguish failures from successful output.
pub(super) async fn handle_command(
    command: &str,
    client: Peer<RoleServer>,
) -> Result<String, String> {
    // Resolve working directory from MCP client roots or process CWD.
    let root_path = match super::path::get_root_directory(&client).await {
        Some(r) => r,
        None => match std::env::current_dir() {
            Ok(cwd) => cwd.to_string_lossy().into_owned(),
            Err(e) => {
                tracing::debug!(error = ?e, "mcp handler: cannot get current directory");
                return Err("error: cannot determine working directory".to_string());
            }
        },
    };

    // Build containment root.
    let containment_root = o8v_fs::ContainmentRoot::new(&root_path).map_err(|e| {
        tracing::debug!("mcp handler: cannot create containment root: {e}");
        "error: cannot create containment root — invalid directory".to_string()
    })?;

    // Parse and dispatch.
    let parsed_command = super::parse::parse_mcp_command(command, &containment_root)?;

    match crate::commands::dispatch_command(
        parsed_command,
        o8v_core::caller::Caller::Mcp,
        &super::INTERRUPTED,
    )
    .await
    {
        Ok((out, _exit, use_stderr)) => {
            if use_stderr {
                Err(out)
            } else {
                Ok(out)
            }
        }
        Err(e) => Err(format!("error: {e}")),
    }
}
