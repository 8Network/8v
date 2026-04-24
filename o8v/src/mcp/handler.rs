// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! MCP interface — parses command string, forwards to dispatch. No logic here.

use base64::Engine;
use rmcp::model::Content;
use rmcp::{Peer, RoleServer};

/// Parse image dimensions from the decoded bytes; return `true` only if we
/// can confirm both sides meet the Vision API's minimum. Unparseable → false
/// (safer to downgrade to text+base64 than to hand the API something it will
/// reject with an opaque 400).
fn is_vision_safe(mime: &str, b64_data: &str) -> bool {
    let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(b64_data) else {
        return false;
    };
    match o8v_core::mime::image_dimensions(&bytes, mime) {
        Some((w, h)) => {
            w >= o8v_core::mime::MIN_IMAGE_DIMENSION && h >= o8v_core::mime::MIN_IMAGE_DIMENSION
        }
        None => false,
    }
}

// ─── Output Cap ──────────────────────────────────────────────────────────────

/// Default MCP output cap (chars). Provides a safety margin below the
/// ~57,000-char persist threshold observed in Claude Code MCP transport.
const DEFAULT_OUTPUT_CAP: usize = 50_000;

/// Cached cap value for the process lifetime.
///
/// Stores `Ok(cap)` on valid configuration, `Err(message)` on invalid override.
/// `OnceLock` is `Send + Sync` — safe for multi-threaded Tokio runtimes.
static MCP_OUTPUT_CAP: std::sync::OnceLock<Result<usize, String>> = std::sync::OnceLock::new();

/// Resolve the MCP output cap.
///
/// Reads `O8V_MCP_OUTPUT_CAP` on first call and caches the result for the
/// process lifetime. If the env var is absent, returns `DEFAULT_OUTPUT_CAP`.
/// Any invalid value (zero, negative, non-numeric, empty string) returns
/// `Err` with a message naming the env var.
fn get_output_cap() -> Result<usize, String> {
    MCP_OUTPUT_CAP
        .get_or_init(|| match std::env::var("O8V_MCP_OUTPUT_CAP") {
            Err(_) => Ok(DEFAULT_OUTPUT_CAP),
            Ok(val) => {
                if val.is_empty() {
                    return Err(
                        "error: O8V_MCP_OUTPUT_CAP is set but empty — must be a positive integer"
                            .to_string(),
                    );
                }
                match val.parse::<i64>() {
                    Ok(n) if n > 0 => Ok(n as usize),
                    Ok(_) => Err(format!(
                        "error: O8V_MCP_OUTPUT_CAP={val:?} is not a positive integer — must be > 0"
                    )),
                    Err(_) => Err(format!(
                        "error: O8V_MCP_OUTPUT_CAP={val:?} is not a valid integer"
                    )),
                }
            }
        })
        .clone()
}

/// Build the structured error message for oversized output (§6 template).
fn oversized_error(output_chars: usize, cap: usize, command: &str) -> String {
    format!(
        "error: output too large for MCP transport\n  output:  {output_chars} chars\n  cap:     {cap} chars (override: O8V_MCP_OUTPUT_CAP)\n  command: {command}\n\nUse a line range instead of --full:\n  8v read <path>:<start>-<end>\nOr read the symbol map first:\n  8v read <path>"
    )
}

// ─── Agent Info ──────────────────────────────────────────────────────────────

fn extract_agent_info(client: &Peer<RoleServer>) -> Option<o8v_core::caller::AgentInfo> {
    let params = client.peer_info()?;
    let mut capabilities = Vec::new();
    if params.capabilities.roots.is_some() {
        capabilities.push("roots".to_string());
    }
    if params.capabilities.sampling.is_some() {
        capabilities.push("sampling".to_string());
    }
    if params.capabilities.elicitation.is_some() {
        capabilities.push("elicitation".to_string());
    }
    Some(o8v_core::caller::AgentInfo {
        name: params.client_info.name.clone(),
        version: params.client_info.version.clone(),
        protocol_version: params.protocol_version.to_string(),
        capabilities,
    })
}

// ─── Command Handler ──────────────────────────────────────────────────────────

/// Parse and execute an 8v command.
///
/// Returns `Ok(text)` on success, `Err(text)` on failure. The MCP tool macro
/// converts `Err` into a `CallToolResult` with `is_error: true`, so the agent
/// can distinguish failures from successful output.
pub(super) async fn handle_command(
    command: &str,
    client: Peer<RoleServer>,
) -> Result<Vec<Content>, String> {
    // Validate cap configuration before doing any work. Invalid O8V_MCP_OUTPUT_CAP
    // returns an observable error immediately (§3, §9 Test 4).
    let cap = get_output_cap()?;

    let agent_info = extract_agent_info(&client);

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
    let (parsed_command, argv) = match super::parse::parse_mcp_command(command, &containment_root)?
    {
        super::parse::ParseOutcome::Parsed(cmd, argv) => (cmd, argv),
        // Help and version output is success content — return it as Ok so the
        // MCP caller does not wrap it in an error envelope.
        super::parse::ParseOutcome::HelpOutput(text) => return Ok(vec![Content::text(text)]),
    };

    // Pre-flight check: abort before reading if metadata sum × 1.20 > cap.
    // Only applies to `read --full` (§4). Cheap metadata reads, no content loaded.
    if let crate::commands::Command::Read(args) = &parsed_command {
        if args.full {
            let mut total_bytes: u64 = 0;
            let mut file_sizes: Vec<(String, u64)> = Vec::new();

            for path_arg in &args.paths {
                // Strip any :start-end suffix before stat (parse_path_range is private;
                // replicate the colon+digit heuristic here).
                let path_str = strip_range_suffix(path_arg);
                if let Ok(meta) = std::fs::metadata(path_str) {
                    let sz = meta.len();
                    total_bytes += sz;
                    file_sizes.push((path_str.to_string(), sz));
                }
                // If metadata fails, skip — dispatch will surface the real error.
            }

            let estimated_chars = (total_bytes as f64 * 1.20) as usize;
            if estimated_chars > cap {
                let mut msg = format!(
                    "error: output too large for MCP transport\n  output:  ~{estimated_chars} chars (estimated)\n  cap:     {cap} chars (override: O8V_MCP_OUTPUT_CAP)\n  command: {command}\n"
                );
                msg.push_str("\nFiles and sizes:\n");
                for (p, sz) in &file_sizes {
                    msg.push_str(&format!("  {p}: {sz} bytes\n"));
                }
                msg.push_str(
                    "\nUse a line range instead of --full:\n  8v read <path>:<start>-<end>\nOr read the symbol map first:\n  8v read <path>",
                );
                return Err(msg);
            }
        }
    }

    // Read takes a typed path so binary content becomes ImageContent /
    // text+base64 instead of a single text block. Event recording is preserved
    // — `crate::dispatch::dispatch` emits the lifecycle events.
    if let crate::commands::Command::Read(args) = parsed_command {
        return dispatch_read_mcp(args, argv, agent_info, cap, command).await;
    }

    match crate::commands::dispatch_command_with_agent(
        parsed_command,
        o8v_core::caller::Caller::Mcp,
        argv,
        &super::INTERRUPTED,
        agent_info,
        o8v_core::render::Audience::Agent,
    )
    .await
    {
        Ok((out, _exit, use_stderr)) => {
            // Post-render safety net: replace any oversized output with a structured
            // error before returning. Wraps both return paths before use_stderr branch (§5).
            if out.len() > cap {
                return Err(oversized_error(out.len(), cap, command));
            }
            if use_stderr {
                Err(out)
            } else {
                Ok(vec![Content::text(out)])
            }
        }
        Err(e) => Err(format!("error: {e}")),
    }
}

/// Typed MCP dispatch for `read` — produces per-entry content blocks so
/// readable binaries surface as `Content::image` rather than base64-in-text.
///
/// PDFs are delivered as text+base64 pending a round-trip check that Claude
/// renders `EmbeddedResource` for `application/pdf`. See design
/// `docs/design/read-non-code-files-l1.md` §4.5.
async fn dispatch_read_mcp(
    args: crate::commands::read::Args,
    argv: Vec<String>,
    agent_info: Option<o8v_core::caller::AgentInfo>,
    cap: usize,
    command: &str,
) -> Result<Vec<Content>, String> {
    use o8v_core::render::read_report::{MultiResult, ReadReport};

    super::INTERRUPTED.store(false, std::sync::atomic::Ordering::Release);
    let mut ctx = crate::dispatch::build_context(&super::INTERRUPTED);
    if let Some(info) = agent_info {
        ctx.extensions.insert(info);
    }

    let cmd = crate::commands::read::ReadCommand { args };

    let (_output, _task_id, report) = match crate::dispatch::dispatch(
        &cmd,
        &mut ctx,
        o8v_core::render::Audience::Agent,
        o8v_core::caller::Caller::Mcp,
        "read",
        &argv,
    )
    .await
    {
        Ok(triple) => triple,
        Err(e) => return Err(format!("error: {e}")),
    };

    let blocks = match report {
        ReadReport::Multi { entries } => {
            let mut out = Vec::with_capacity(entries.len());
            for entry in entries {
                out.push(Content::text(format!("=== {} ===", entry.label)));
                match entry.result {
                    MultiResult::Ok { report } => out.extend(report_to_blocks(*report, cap)?),
                    MultiResult::Err { message } => {
                        out.push(Content::text(format!("error: {message}")));
                    }
                }
            }
            out
        }
        other => report_to_blocks(other, cap)?,
    };

    let _ = command;
    Ok(blocks)
}

/// Map a single (non-Multi) `ReadReport` to MCP content blocks. Applies the
/// text-output cap only to text blocks — image/resource payloads are exempt,
/// bounded by `FsConfig::max_file_size` instead.
fn report_to_blocks(
    report: o8v_core::render::read_report::ReadReport,
    cap: usize,
) -> Result<Vec<Content>, String> {
    use o8v_core::render::read_report::ReadReport;
    use o8v_core::render::Renderable;

    match report {
        ReadReport::BinaryContent {
            path: _,
            mime_type,
            size_bytes,
            base64,
        } => {
            if mime_type.starts_with("image/") && is_vision_safe(&mime_type, &base64) {
                Ok(vec![Content::image(base64, mime_type)])
            } else {
                // Images below the Vision API's minimum dimensions, images we
                // can't parse, and non-image binaries (e.g. PDF) are delivered
                // as text+base64. Handing an undersized image to Claude as
                // `ImageContent` poisons the turn with an opaque 400 — safer
                // to downgrade than to guess. See design §4.5.
                let text = format!("{mime_type}, {size_bytes} bytes\nbase64: {base64}");
                Ok(vec![Content::text(text)])
            }
        }
        other => {
            let text = other.render_plain().into_string();
            if text.len() > cap {
                return Err(oversized_error(text.len(), cap, "read"));
            }
            Ok(vec![Content::text(text)])
        }
    }
}

/// Strip a `:start-end` range suffix from a path argument, replicating the
/// heuristic in `read.rs:parse_path_range` without calling the private function.
///
/// Returns the path portion only (borrowed slice).
fn strip_range_suffix(input: &str) -> &str {
    if let Some(colon_pos) = input.rfind(':') {
        let range_part = &input[colon_pos + 1..];
        if let Some(dash_pos) = range_part.find('-') {
            let start_ok = range_part[..dash_pos].parse::<usize>().is_ok();
            let end_ok = range_part[dash_pos + 1..].parse::<usize>().is_ok();
            if start_ok && end_ok {
                return &input[..colon_pos];
            }
        }
    }
    input
}
