// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! MCP server for 8v — expose check and fmt as tools.

mod handler;
mod parse;
pub(crate) mod path;

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, Content, Meta, ServerCapabilities, ServerInfo};
use rmcp::{tool, tool_handler, tool_router, Peer, RoleServer, ServerHandler, ServiceExt};
use schemars::JsonSchema;
use serde::Deserialize;
use std::sync::atomic::AtomicBool;

pub(super) static INTERRUPTED: AtomicBool = AtomicBool::new(false);

#[derive(Deserialize, JsonSchema)]
struct CommandParams {
    /// The 8v command to run, e.g. `8v read src/main.rs` or `8v check .`.
    command: String,
}

#[derive(Clone)]
struct EightVServer {
    tool_router: ToolRouter<Self>,
}

impl EightVServer {
    fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }
}

#[tool_router]
impl EightVServer {
    #[tool(name = "8v")]
    #[doc = include_str!("instructions.txt")]
    async fn run_command(
        &self,
        Parameters(params): Parameters<CommandParams>,
        _meta: Meta,
        client: Peer<RoleServer>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        match handler::handle_command(&params.command, client).await {
            Ok(blocks) => Ok(CallToolResult::success(blocks)),
            Err(msg) => Ok(CallToolResult::error(vec![Content::text(msg)])),
        }
    }
}

#[tool_handler]
impl ServerHandler for EightVServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
    }
}

/// Start the MCP server on stdio transport.
pub async fn serve() -> Result<(), Box<dyn std::error::Error>> {
    let server = EightVServer::new();
    let transport = rmcp::transport::io::stdio();
    server.serve(transport).await?.waiting().await?;
    Ok(())
}
