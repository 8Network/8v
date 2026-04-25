// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Smoke test: verify that ProfileArtifacts actually differ across profiles.
//!
//! Writes assembled CLAUDE.md and .mcp.json to /tmp/8v-profile-dump/<profile>/
//! without spawning any agent (no API spend).

use std::path::Path;

use o8v_testkit::benchmark::{claude::assemble_agent_context, profiles::ToolProfile, Agent};

/// Workspace dir passed to setup(). Setup for all three profiles ignores it,
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

    // ── Dump all three profiles ───────────────────────────────────────────────
    dump_profile(ToolProfile::Native, "native", baseline_claude_md);
    dump_profile(ToolProfile::EightV, "eightv", baseline_claude_md);
    dump_profile(ToolProfile::Caveman, "caveman", baseline_claude_md);
    dump_profile(ToolProfile::ToolSearch, "tool-search", baseline_claude_md);
    dump_profile(ToolProfile::Mcp2cli, "mcp2cli", baseline_claude_md);

    // ── Load the dumps ────────────────────────────────────────────────────────
    let native_claude = std::fs::read_to_string(format!("{DUMP_ROOT}/native/CLAUDE.md"))
        .expect("read native CLAUDE.md");
    let eightv_claude = std::fs::read_to_string(format!("{DUMP_ROOT}/eightv/CLAUDE.md"))
        .expect("read eightv CLAUDE.md");
    let caveman_claude = std::fs::read_to_string(format!("{DUMP_ROOT}/caveman/CLAUDE.md"))
        .expect("read caveman CLAUDE.md");

    let native_mcp: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(format!("{DUMP_ROOT}/native/mcp.json")).unwrap(),
    )
    .unwrap();
    let eightv_mcp: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(format!("{DUMP_ROOT}/eightv/mcp.json")).unwrap(),
    )
    .unwrap();
    let caveman_mcp: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(format!("{DUMP_ROOT}/caveman/mcp.json")).unwrap(),
    )
    .unwrap();

    // ── Common-base substring present in all profiles ─────────────────────────
    // "Rules for the AI" is a distinctive heading from the common_base_claude.md.
    let common_marker = "Rules for the AI";
    assert!(
        native_claude.contains(common_marker),
        "Native CLAUDE.md must contain common-base marker '{common_marker}'"
    );
    assert!(
        eightv_claude.contains(common_marker),
        "EightV CLAUDE.md must contain common-base marker '{common_marker}'"
    );
    assert!(
        caveman_claude.contains(common_marker),
        "Caveman CLAUDE.md must contain common-base marker '{common_marker}'"
    );

    // ── Invariant: Native ─────────────────────────────────────────────────────
    // Native profile should not inject any tool-specific content.
    assert!(
        !native_claude.to_lowercase().contains("caveman"),
        "Native CLAUDE.md must NOT contain 'caveman': got {} bytes",
        native_claude.len()
    );
    assert!(
        native_mcp.get("mcpServers").map(|v| v.get("8v")).is_none()
            || native_mcp["mcpServers"].get("8v").is_none(),
        "Native .mcp.json must NOT contain an '8v' server key"
    );
    // Native CLAUDE.md = common_base (4904) + "\n\n" + task baseline (35) ≈ 4941 bytes.
    let common_base_size: usize = 4904;
    let task_baseline_size: usize = baseline_claude_md.len();
    let expected_native_min = common_base_size + task_baseline_size;
    assert!(
        native_claude.len() >= expected_native_min,
        "Native CLAUDE.md ({} bytes) must be >= common_base ({common_base_size}) + task_baseline ({task_baseline_size})",
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
    // EightV does not prepend CLAUDE.md — baseline should be unchanged.
    assert_eq!(
        eightv_claude, native_claude,
        "EightV CLAUDE.md should equal Native CLAUDE.md (no prepend)"
    );

    // ── Invariant: Caveman ────────────────────────────────────────────────────
    // Caveman skill asset starts with "---\nname: caveman"
    assert!(
        caveman_claude.contains("name: caveman"),
        "Caveman CLAUDE.md must contain 'name: caveman' from skill asset"
    );
    // Caveman CLAUDE.md = caveman_skill (3653) + common_base (4904) + task_baseline (35).
    let caveman_skill_size: usize = 3653;
    let expected_caveman_min = caveman_skill_size + common_base_size + task_baseline_size;
    assert!(
        caveman_claude.len() >= expected_caveman_min,
        "Caveman CLAUDE.md ({} bytes) must be >= caveman_skill ({caveman_skill_size}) + common_base ({common_base_size}) + task_baseline ({task_baseline_size})",
        caveman_claude.len()
    );
    // Caveman has no MCP server injection.
    assert!(
        caveman_mcp.get("mcpServers").map(|v| v.get("8v")).is_none()
            || caveman_mcp
                .get("mcpServers")
                .and_then(|v| v.get("8v"))
                .is_none(),
        "Caveman .mcp.json must NOT contain '8v' server key"
    );

    // ── Invariant: ToolSearch ─────────────────────────────────────────────────
    let tool_search_claude = std::fs::read_to_string(format!("{DUMP_ROOT}/tool-search/CLAUDE.md"))
        .expect("read tool-search CLAUDE.md");
    let tool_search_mcp: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(format!("{DUMP_ROOT}/tool-search/mcp.json")).unwrap(),
    )
    .unwrap();

    // ToolSearch sets only an env var — no CLAUDE.md prepend, no MCP injection.
    assert_eq!(
        tool_search_claude, native_claude,
        "ToolSearch CLAUDE.md should equal Native CLAUDE.md (no prepend)"
    );
    assert!(
        tool_search_mcp
            .get("mcpServers")
            .and_then(|v| v.get("8v"))
            .is_none(),
        "ToolSearch .mcp.json must NOT contain '8v' server key"
    );

    // Verify artifacts directly: env must contain ENABLE_TOOL_SEARCH=true,
    // no mcp_json_fragment, no claude_md_prepend.
    let workspace = Path::new(WORKSPACE);
    let ts_harness = ToolProfile::ToolSearch.harness();
    let ts_artifacts = ts_harness
        .setup(workspace, o8v_testkit::benchmark::Agent::Claude)
        .expect("ToolSearch setup must not fail");
    assert_eq!(
        ts_artifacts
            .env
            .get("ENABLE_TOOL_SEARCH")
            .map(|s| s.as_str()),
        Some("true"),
        "ToolSearch artifacts must have ENABLE_TOOL_SEARCH=true"
    );
    assert!(
        ts_artifacts.mcp_json_fragment.is_none(),
        "ToolSearch artifacts must have no mcp_json_fragment"
    );
    assert!(
        ts_artifacts.claude_md_prepend.is_none(),
        "ToolSearch artifacts must have no claude_md_prepend"
    );

    // ── Invariant: Mcp2cli ────────────────────────────────────────────────────
    let mcp2cli_claude = std::fs::read_to_string(format!("{DUMP_ROOT}/mcp2cli/CLAUDE.md"))
        .expect("read mcp2cli CLAUDE.md");
    let mcp2cli_mcp: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(format!("{DUMP_ROOT}/mcp2cli/mcp.json")).unwrap(),
    )
    .unwrap();

    // mcp2cli is CLI-based — CLAUDE.md prepend only, no MCP server injection.
    assert!(
        mcp2cli_claude.contains("mcp2cli"),
        "Mcp2cli CLAUDE.md must contain 'mcp2cli' skill marker"
    );
    assert_ne!(
        mcp2cli_claude, native_claude,
        "Mcp2cli CLAUDE.md must differ from Native (has prepend)"
    );
    assert!(
        mcp2cli_mcp
            .get("mcpServers")
            .and_then(|v| v.get("8v"))
            .is_none(),
        "Mcp2cli .mcp.json must NOT contain '8v' server key"
    );

    eprintln!("\nByte sizes:");
    eprintln!("  Native  CLAUDE.md: {} bytes", native_claude.len());
    eprintln!("  EightV  CLAUDE.md: {} bytes", eightv_claude.len());
    eprintln!("  Caveman CLAUDE.md: {} bytes", caveman_claude.len());
    eprintln!(
        "  Delta (Caveman - Native): {} bytes",
        caveman_claude.len() - native_claude.len()
    );
}
