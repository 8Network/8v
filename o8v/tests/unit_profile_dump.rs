// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Smoke test: verify that ProfileArtifacts actually differ across profiles.
//!
//! Writes assembled CLAUDE.md and .mcp.json to /tmp/8v-profile-dump/<profile>/
//! without spawning any agent (no API spend).

use std::path::Path;

use o8v_testkit::benchmark::{claude::assemble_agent_context, profiles::ToolProfile, Agent};

/// Workspace dir passed to setup(). Setup for both profiles ignores it,
/// but we need a real Path.
const WORKSPACE: &str = "/tmp/8v-profile-dump-workspace";
const DUMP_ROOT: &str = "/tmp/8v-profile-dump";

fn dump_profile(profile: ToolProfile, profile_id: &str, baseline_claude_md: &str) {
    let workspace = Path::new(WORKSPACE);
    std::fs::create_dir_all(workspace).expect("create workspace dir");

    let harness = profile.harness();
    let artifacts = match harness.setup(workspace, Agent::Claude) {
        Ok(a) => a,
        Err(e) => panic!("setup({profile_id}) failed: {e}"),
    };

    let ctx = match assemble_agent_context(baseline_claude_md, serde_json::json!({}), &artifacts) {
        Ok(c) => c,
        Err(e) => panic!("assemble_agent_context({profile_id}) failed: {e}"),
    };

    let out_dir = Path::new(DUMP_ROOT).join(profile_id);
    std::fs::create_dir_all(&out_dir).expect("create dump dir");

    std::fs::write(out_dir.join("CLAUDE.md"), &ctx.claude_md).expect("write CLAUDE.md");

    let mcp_str = serde_json::to_string_pretty(&ctx.mcp_json).expect("serialize mcp_json");
    std::fs::write(out_dir.join("mcp.json"), &mcp_str).expect("write mcp.json");

    eprintln!(
        "[dump] {profile_id}: CLAUDE.md={} bytes, mcp.json={} bytes",
        ctx.claude_md.len(),
        mcp_str.len()
    );
}

#[test]
fn profile_dump_smoke() {
    // Simulate a baseline CLAUDE.md already in the workspace (non-empty, realistic).
    let baseline_claude_md = "# Project\n\nThis is a test project.\n";

    // ── Dump both profiles ────────────────────────────────────────────────────
    dump_profile(ToolProfile::Native, "native", baseline_claude_md);
    dump_profile(ToolProfile::EightV, "eightv", baseline_claude_md);

    // ── Load the dumps ────────────────────────────────────────────────────────
    let native_claude = std::fs::read_to_string(format!("{DUMP_ROOT}/native/CLAUDE.md"))
        .expect("read native CLAUDE.md");
    let eightv_claude = std::fs::read_to_string(format!("{DUMP_ROOT}/eightv/CLAUDE.md"))
        .expect("read eightv CLAUDE.md");

    let native_mcp: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(format!("{DUMP_ROOT}/native/mcp.json")).unwrap(),
    )
    .unwrap();
    let eightv_mcp: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(format!("{DUMP_ROOT}/eightv/mcp.json")).unwrap(),
    )
    .unwrap();

    // ── Common-base substring present in all profiles ─────────────────────────
    let common_marker = "software engineer";
    assert!(
        native_claude.contains(common_marker),
        "Native CLAUDE.md must contain common-base marker '{common_marker}'"
    );
    assert!(
        eightv_claude.contains(common_marker),
        "EightV CLAUDE.md must contain common-base marker '{common_marker}'"
    );

    // ── Invariant: Native ─────────────────────────────────────────────────────
    // Native profile should not inject any tool-specific content.
    assert!(
        native_mcp.get("mcpServers").map(|v| v.get("8v")).is_none()
            || native_mcp["mcpServers"].get("8v").is_none(),
        "Native .mcp.json must NOT contain an '8v' server key"
    );
    let task_baseline_size: usize = baseline_claude_md.len();
    assert!(
        native_claude.len() > task_baseline_size,
        "Native CLAUDE.md ({} bytes) must include common_base on top of task_baseline ({task_baseline_size})",
        native_claude.len()
    );

    // ── Invariant: EightV ─────────────────────────────────────────────────────
    let eightv_servers = eightv_mcp
        .get("mcpServers")
        .expect("EightV .mcp.json must have 'mcpServers'");
    assert!(
        eightv_servers.get("8v").is_some(),
        "EightV .mcp.json must contain '8v' server key"
    );
    // EightV prepends a tool directive — must be larger than native and
    // mention the 8v MCP tool.
    assert!(
        eightv_claude.len() > native_claude.len(),
        "EightV CLAUDE.md ({} bytes) must be larger than Native ({} bytes) due to prepend",
        eightv_claude.len(),
        native_claude.len()
    );
    assert!(
        eightv_claude.contains("mcp__8v__8v"),
        "EightV CLAUDE.md must reference the 8v MCP tool"
    );

    eprintln!("\nByte sizes:");
    eprintln!("  Native  CLAUDE.md: {} bytes", native_claude.len());
    eprintln!("  EightV  CLAUDE.md: {} bytes", eightv_claude.len());
    eprintln!(
        "  Delta (EightV - Native): {} bytes",
        eightv_claude.len().saturating_sub(native_claude.len())
    );
}
