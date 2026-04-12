// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! MCP server for 8v — expose check and fmt as tools.

mod handler;
mod parse;
mod path;

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{Meta, ServerCapabilities, ServerInfo};
use rmcp::{tool, tool_handler, tool_router, Peer, RoleServer, ServerHandler, ServiceExt};
use schemars::JsonSchema;
use serde::Deserialize;
use std::sync::atomic::AtomicBool;

pub(super) static INTERRUPTED: AtomicBool = AtomicBool::new(false);

#[derive(Deserialize, JsonSchema)]
struct CommandParams {
    /// The 8v command to run. Examples:
    /// - `8v read src/main.rs` (symbol map)
    /// - `8v read src/main.rs:10-20` (line range)
    /// - `8v write src/main.rs:15 "new content"` (replace line)
    /// - `8v write src/main.rs --find "old" --replace "new"`
    /// - `8v check .` (run all checks)
    /// - `8v fmt .` (format all files)
    /// - `8v test .` (run tests)
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
    #[tool(
        name = "8v",
        description = "8v — code reliability tool. One command for check, format, read, write, build, and run.\n\n## Read files\n`8v read <path>` — symbol map (functions, structs, classes). See the structure first.\n`8v read <path>:<start>-<end>` — specific line range. Use after seeing symbols.\n`8v read <path> --full` — entire file content.\n`8v read <path> --json` — JSON output.\n\n## Write files\n`8v write <path>:<line> \"<content>\"` — replace a single line.\n`8v write <path>:<start>-<end> \"<content>\"` — replace a line range.\n`8v write <path>:<line> --insert \"<content>\"` — insert before a line.\n`8v write <path>:<start>-<end> --delete` — delete lines.\n`8v write <path> --find \"<old>\" --replace \"<new>\"` — find and replace.\n`8v write <path> --append \"<content>\"` — append to file.\n\n## Check and format\n`8v check .` — run all checks (lint, type check, format check).\n`8v fmt .` — auto-format all files.\n\n## Test\n`8v test .` — run project tests (cargo test, go test, etc.).\n`8v test . --json` — JSON output with exit code, stdout, stderr, duration.\n\n## Build\n`8v build .` — build the project (cargo build, go build, dotnet build, etc.).\n`8v build . --json` — JSON output with exit code, stdout, stderr, duration, stack.\n`8v build . --timeout 300` — override default 300s timeout.\n\n## Run commands\n`8v run \"<command>\"` — execute a command with timeout and structured output.\n`8v run \"<command>\" --json` — JSON output.\n`8v run \"<command>\" --timeout 120` — override default 120s timeout.\n\n## Search files\n`8v search <pattern>` — find content across files (respects .gitignore).\n`8v search <pattern> --files` — find files by name instead of content.\n`8v search <pattern> -i` — case-insensitive.\n`8v search <pattern> -e rs` — filter by extension.\n`8v search <pattern> -C 2` — context lines (default: 2).\n`8v search <pattern> --limit 20` — max files with matches (default: 20).\n`8v search <pattern> [path]` — search in a specific subdirectory.\n\n## List files\n`8v ls` — list all projects (name, stack, path). Use first to understand the repo.\n`8v ls --tree` — full file hierarchy with project labels. Replaces multiple Glob calls.\n`8v ls --files` — flat file listing, one per line.\n`8v ls --depth N` — limit tree depth (1 = top-level dirs only).\n`8v ls --match pattern` — filter files by glob pattern (e.g. \"*_test*\").\n`8v ls --loc` — show line counts per file. Helps decide what to read.\n`8v ls --stack name` — filter to projects of a specific stack (e.g. rust, go, python).\n`8v ls [path] --tree` — tree of a specific subdirectory.\n\n## Workflow\n1. `8v ls` — discover projects and structure first.\n2. `8v ls --tree --loc` — understand file hierarchy and sizes.\n3. `8v read <path>` — see symbols (cheapest file read).\n4. Read only the lines you need.\n5. `8v search <pattern>` — find content across files.\n6. `8v write` to make changes.\n7. `8v test .` to run tests.\n8. `8v build .` to compile.\n9. `8v check .` to verify all checks pass.\n10. `8v run \"<command>\"` for any other command."
    )]
    async fn run_command(
        &self,
        Parameters(params): Parameters<CommandParams>,
        _meta: Meta,
        client: Peer<RoleServer>,
    ) -> String {
        handler::handle_command(&params.command, client).await
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
