// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Counterexample sweep — `8v hook` runtime behaviour and `8v init --yes` installer.
//!
//! Naming convention:
//!   `_bug`      — expected to FAIL on current code (confirms a real defect)
//!   `_holds`    — expected to PASS (confirms an invariant)
//!
//! Rules:
//! * No production source files are edited.
//! * No bugs are fixed here — document only.
//! * Every test runs without `#[ignore]`.
//! * All E2E invocations use `_8V_HOME` for storage isolation.
//!
//! Area A: runtime (8v hook pre / post)
//! Area B: installer (8v init --yes → setup_claude_settings)
//!
//! Dropped vectors (not testable via binary):
//!   - install_claude_hooks idempotency: guarded by `!args.yes`; no binary entry
//!     point reachable without an interactive terminal.
//!   - similar-named hook deduplication: same constraint.

use std::fs;
use std::io::Write as _;
use std::process::{Command, Stdio};
use tempfile::TempDir;

// ─── Helpers ────────────────────────────────────────────────────────────────

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_8v"))
}

/// Isolated HOME directory. Storage lands at `<dir>/.8v/`.
fn home() -> TempDir {
    TempDir::new().expect("create temp dir")
}

/// Parse `<home>/.8v/events.ndjson` into JSON values.
fn read_events(dir: &TempDir) -> Vec<serde_json::Value> {
    let path = dir.path().join(".8v").join("events.ndjson");
    if !path.exists() {
        return Vec::new();
    }
    let content = fs::read_to_string(&path).expect("read events.ndjson");
    content
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| serde_json::from_str(l).expect("parse event line"))
        .collect()
}

fn run_pre(dir: &TempDir, payload: &str) -> std::process::Output {
    let mut child = bin()
        .args(["hook", "pre"])
        .env("_8V_HOME", dir.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn 8v hook pre");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(payload.as_bytes())
        .expect("write stdin");
    child.wait_with_output().expect("wait for 8v hook pre")
}

fn run_post(dir: &TempDir, payload: &str) -> std::process::Output {
    let mut child = bin()
        .args(["hook", "post"])
        .env("_8V_HOME", dir.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn 8v hook post");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(payload.as_bytes())
        .expect("write stdin");
    child.wait_with_output().expect("wait for 8v hook post")
}

fn pre_payload(session_id: &str, tool_use_id: &str, tool_name: &str, command: &str) -> String {
    // The command string is embedded raw; callers must not include bare `"` chars.
    format!(
        r#"{{"hook_event_name":"PreToolUse","session_id":"{session_id}","tool_use_id":"{tool_use_id}","tool_name":"{tool_name}","tool_input":{{"command":"{command}"}}}}"#
    )
}

fn post_payload(session_id: &str, tool_use_id: &str, tool_name: &str, command: &str) -> String {
    format!(
        r#"{{"hook_event_name":"PostToolUse","session_id":"{session_id}","tool_use_id":"{tool_use_id}","tool_name":"{tool_name}","tool_input":{{"command":"{command}"}},"tool_response":{{"output":"ok"}}}}"#
    )
}

// ─── Area A — runtime ────────────────────────────────────────────────────────

/// BUG: Sending `8v hook pre` twice with the same (session_id, tool_use_id)
/// should produce 3 events: two `CommandStarted` and one `CommandCompleted`.
/// Current code silently OVERWRITES the correlation temp file on the second
/// call, so the first `CommandStarted` is permanently orphaned — its run_id
/// will never appear in a `CommandCompleted`. Then a single `8v hook post`
/// emits `CommandCompleted` only for the second run_id. The first run_id is
/// lost. The test asserts the invariant that SHOULD hold (3 events), which
/// exposes the defect.
#[test]
fn duplicate_pre_orphans_first_started_bug() {
    let dir = home();
    let session = "claude_session_dup_pre";
    let tool_use_id = "toolu_dup_001";

    // Fire pre twice — same session + tool_use_id.
    run_pre(&dir, &pre_payload(session, tool_use_id, "Bash", "ls"));
    run_pre(&dir, &pre_payload(session, tool_use_id, "Bash", "ls"));

    // Fire post once — only pairs with the second pre (second run_id).
    run_post(&dir, &post_payload(session, tool_use_id, "Bash", "ls"));

    let events = read_events(&dir);

    // Should be: CommandStarted (first), CommandStarted (second), CommandCompleted.
    // Current code produces only 2 events (overwrites first record), so this
    // assertion will FAIL on unfixed code, confirming the bug.
    assert_eq!(
        events.len(),
        3,
        "duplicate pre must produce two CommandStarted events (one orphaned) + one \
         CommandCompleted; got {} events — first run_id is silently orphaned",
        events.len()
    );
}

/// HOLDS: An empty string `tool_use_id` is a degenerate but legal value.
/// The hook must exit 0 — it must never block the Claude pipeline.
#[test]
fn empty_tool_use_id_pre_exits_zero_holds() {
    let dir = home();
    let out = run_pre(
        &dir,
        &pre_payload("claude_session_empty_id", "", "Bash", "ls"),
    );
    assert!(
        out.status.success(),
        "hook pre with empty tool_use_id must exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// HOLDS: Tool names sent in all-uppercase must be normalised to lowercase
/// in the emitted `command` field.
#[test]
fn tool_name_uppercase_normalised_to_lowercase_holds() {
    let dir = home();
    let out = run_pre(
        &dir,
        &pre_payload("claude_session_upper", "toolu_upper_001", "BASH", "ls"),
    );
    assert!(
        out.status.success(),
        "hook pre must exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let events = read_events(&dir);
    assert_eq!(events.len(), 1, "must emit exactly one event");
    assert_eq!(
        events[0]["command"].as_str().unwrap(),
        "bash",
        "command must be lowercase regardless of tool_name casing; got {:?}",
        events[0]["command"]
    );
}

/// HOLDS: An unrecognised tool name must not crash the hook — exit 0 always.
#[test]
fn unknown_tool_name_exits_zero_holds() {
    let dir = home();
    let out = run_pre(
        &dir,
        &pre_payload(
            "claude_session_unknown",
            "toolu_unknown_001",
            "FutureTool42",
            "some-arg",
        ),
    );
    assert!(
        out.status.success(),
        "hook pre with unknown tool name must exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let events = read_events(&dir);
    assert_eq!(
        events.len(),
        1,
        "unknown tool name must still emit one event"
    );
    assert_eq!(events[0]["event"].as_str().unwrap(), "CommandStarted");
}

/// HOLDS: A very large command string (50 KB) must not crash or hang the hook.
#[test]
fn large_bash_command_exits_zero_holds() {
    let dir = home();
    // Build a 50 KB command that contains no JSON-special characters.
    let large_cmd = "x".repeat(50_000);
    let out = run_pre(
        &dir,
        &pre_payload(
            "claude_session_large",
            "toolu_large_001",
            "Bash",
            &large_cmd,
        ),
    );
    assert!(
        out.status.success(),
        "hook pre with 50 KB command must exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// HOLDS: Any Claude-provided session string must produce a `ses_`-prefixed
/// session_id in the emitted event. The prefix is guaranteed by
/// `SessionId::from_claude_session_id` regardless of input content.
#[test]
fn ses_prefix_on_any_session_string_holds() {
    let inputs = [
        "claude_session_abc",
        "",                       // empty string
        "ses_already_has_prefix", // looks like it already has a prefix
        "UPPERCASE_SESSION_ID",
        "01ARZ3NDEKTSV4RRFFQ69G5FAV", // ULID-shaped input
    ];

    for input in inputs {
        let dir = home();
        let out = run_pre(
            &dir,
            &pre_payload(input, "toolu_sesprefix_001", "Read", "file.rs"),
        );
        assert!(
            out.status.success(),
            "hook pre must exit 0 for session '{input}'; stderr: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let events = read_events(&dir);
        assert_eq!(events.len(), 1, "must emit one event for session '{input}'");
        let session_id = events[0]["session_id"].as_str().unwrap_or("");
        assert!(
            session_id.starts_with("ses_"),
            "session_id must start with ses_ for input '{input}'; got '{session_id}'"
        );
    }
}

/// HOLDS: `8v hook post` paired with a pre must produce exactly one
/// `CommandCompleted` (not two), and the run_id must match the paired
/// `CommandStarted`. This guards against the event store growing on retries.
#[test]
fn post_paired_emits_exactly_one_completed_holds() {
    let dir = home();
    let session = "claude_session_paired_count";
    let tool_use_id = "toolu_paired_count_001";

    run_pre(&dir, &pre_payload(session, tool_use_id, "Edit", "a.rs"));
    run_post(&dir, &post_payload(session, tool_use_id, "Edit", "a.rs"));

    let events = read_events(&dir);
    let completed: Vec<_> = events
        .iter()
        .filter(|e| e["event"].as_str() == Some("CommandCompleted"))
        .collect();

    assert_eq!(
        completed.len(),
        1,
        "paired pre+post must emit exactly one CommandCompleted; got {}",
        completed.len()
    );

    let started_run_id = events
        .iter()
        .find(|e| e["event"].as_str() == Some("CommandStarted"))
        .and_then(|e| e["run_id"].as_str())
        .expect("CommandStarted must be present");

    let completed_run_id = completed[0]["run_id"].as_str().unwrap_or("");
    assert_eq!(
        started_run_id, completed_run_id,
        "run_id must match between CommandStarted and CommandCompleted"
    );
}

// ─── Area B — installer (setup_claude_settings via `8v init --yes`) ──────────

/// Helper: run `8v init --yes <path>` with a fake MCP binary name so the init
/// does not mutate real user config. Returns the process Output.
fn run_init_yes(project_dir: &TempDir) -> std::process::Output {
    bin()
        .args([
            "init",
            "--yes",
            "--mcp-command",
            "8v",
            project_dir.path().to_str().unwrap(),
        ])
        .env("_8V_HOME", project_dir.path())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("spawn 8v init --yes")
}

/// Read `.claude/settings.json` from a project dir.
fn read_settings(dir: &TempDir) -> serde_json::Value {
    let path = dir.path().join(".claude").join("settings.json");
    let content = fs::read_to_string(&path).expect("read .claude/settings.json");
    serde_json::from_str(&content).expect("parse settings.json")
}

/// HOLDS: Running `8v init --yes` twice must not duplicate the MCP permission.
/// `mcp__8v__8v` must appear exactly once in `permissions.allow`.
#[test]
fn init_yes_twice_no_permission_duplicates_holds() {
    let dir = home();

    // First run
    let out1 = run_init_yes(&dir);
    assert!(
        out1.status.success(),
        "first 8v init --yes must exit 0; stderr: {}",
        String::from_utf8_lossy(&out1.stderr)
    );

    // Second run — idempotency check
    let out2 = run_init_yes(&dir);
    assert!(
        out2.status.success(),
        "second 8v init --yes must exit 0; stderr: {}",
        String::from_utf8_lossy(&out2.stderr)
    );

    let settings = read_settings(&dir);
    let allow = settings["permissions"]["allow"]
        .as_array()
        .expect("permissions.allow must be an array");

    let count = allow
        .iter()
        .filter(|v| v.as_str() == Some("mcp__8v__8v"))
        .count();

    assert_eq!(
        count, 1,
        "mcp__8v__8v must appear exactly once in permissions.allow after two init runs; \
         found {count} times. allow array: {allow:?}"
    );
}

/// HOLDS: `8v init --yes` must preserve unknown top-level keys that exist in
/// `.claude/settings.json` before the run. The `#[serde(flatten)]` field in
/// `ClaudeSettings` is supposed to round-trip arbitrary keys.
#[test]
fn init_yes_unknown_top_level_keys_preserved_holds() {
    let dir = home();

    // Pre-seed .claude/settings.json with an unknown key.
    let claude_dir = dir.path().join(".claude");
    fs::create_dir_all(&claude_dir).expect("create .claude dir");
    fs::write(
        claude_dir.join("settings.json"),
        r#"{"customOrg":"acme-corp","permissions":{"allow":[]}}"#,
    )
    .expect("write seed settings.json");

    let out = run_init_yes(&dir);
    assert!(
        out.status.success(),
        "8v init --yes must exit 0 when settings.json has unknown keys; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let settings = read_settings(&dir);
    assert_eq!(
        settings["customOrg"].as_str(),
        Some("acme-corp"),
        "unknown top-level key 'customOrg' must survive 8v init --yes; got: {settings:?}"
    );
}

/// HOLDS: `8v init --yes` must return a non-zero exit code (and print a clear
/// error) when `.claude/settings.json` contains malformed JSON. It must NOT
/// panic or overwrite the file with garbage.
#[test]
fn init_yes_malformed_settings_json_exits_nonzero_holds() {
    let dir = home();

    // Pre-seed with malformed JSON.
    let claude_dir = dir.path().join(".claude");
    fs::create_dir_all(&claude_dir).expect("create .claude dir");
    fs::write(claude_dir.join("settings.json"), b"{this is not json}")
        .expect("write malformed settings.json");

    let out = run_init_yes(&dir);

    assert!(
        !out.status.success(),
        "8v init --yes must exit non-zero when settings.json is malformed; \
         stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    // The malformed file must not have been silently overwritten with valid JSON.
    let raw = fs::read_to_string(dir.path().join(".claude").join("settings.json"))
        .expect("settings.json must still exist");
    assert_eq!(
        raw, "{this is not json}",
        "malformed settings.json must not be overwritten on error; got: {raw:?}"
    );
}

// ── H-1 fail-closed ─────────────────────────────────────────────────────────────

/// Empty stdin at the PreToolUse gate is not a legitimate invocation.
/// Fail-closed: exit 1 (block), not exit 0 (silent allow).
#[test]
fn pre_tool_use_empty_stdin_fails_closed_exit_1() {
    let dir = home();
    let out = run_pre(&dir, "");
    assert_eq!(
        out.status.code(),
        Some(1),
        "empty stdin at Pre gate must exit 1 (block); got: {:?}
stderr: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Malformed JSON at the PreToolUse gate is not a legitimate invocation.
/// Fail-closed: exit 1 (block), not exit 0 (silent allow).
#[test]
fn pre_tool_use_malformed_json_fails_closed_exit_1() {
    let dir = home();
    let out = run_pre(&dir, "this is not json at all");
    assert_eq!(
        out.status.code(),
        Some(1),
        "malformed stdin at Pre gate must exit 1 (block); got: {:?}
stderr: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Valid PreToolUse stdin still exits 0 — the fail-closed behavior must
/// only trigger on invalid input, not on the happy path.
#[test]
fn pre_tool_use_valid_stdin_exits_0() {
    let dir = home();
    let payload = pre_payload("sess-h1-ok", "tu-ok", "Bash", "echo ok");
    let out = run_pre(&dir, &payload);
    assert!(
        out.status.success(),
        "valid Pre stdin must still exit 0 (observability principle preserved); got: {:?}
stderr: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
}

// ── I-3 installed hooks use portable bare `8v` invocation ─────────────────────

/// Generated pre-commit hook must invoke bare `8v` (found via PATH), NOT an
/// absolute path baked in at install time. Absolute paths break on binary
/// moves, CI, and shared dotfiles. Fix: emit `8v hooks git on-commit`.
#[test]
fn i3_git_pre_commit_hook_uses_portable_8v_invocation() {
    let dir = TempDir::new().expect("tempdir");
    // Need a .git dir for pre-commit hook installation to proceed.
    std::fs::create_dir_all(dir.path().join(".git").join("hooks")).expect("mkdir .git/hooks");

    let out = run_init_yes(&dir);
    assert!(
        out.status.success(),
        "8v init --yes must succeed;
stdout: {}
stderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    let hook_path = dir.path().join(".git").join("hooks").join("pre-commit");
    let content = std::fs::read_to_string(&hook_path).expect("read pre-commit");

    // Must invoke bare `8v` — no absolute path baked in.
    let has_bare = content
        .lines()
        .any(|line| line.trim_start().starts_with("8v hooks git on-commit"));
    assert!(
        has_bare,
        "hook must invoke bare `8v hooks git on-commit`, not an absolute path; hook content:\n{content}"
    );

    // Must NOT contain an absolute path to the binary.
    let has_absolute = content.lines().filter(|l| !l.starts_with('#')).any(|line| {
        let cmd = line.split_whitespace().find(|t| *t != "exec").unwrap_or("");
        cmd.starts_with('/') || cmd.starts_with("'/") || cmd.starts_with("\"/")
    });
    assert!(
        !has_absolute,
        "hook must not bake in an absolute binary path; hook content:\n{content}"
    );
}

/// Same guarantee for the commit-msg hook.
#[test]
fn i3_git_commit_msg_hook_uses_portable_8v_invocation() {
    let dir = TempDir::new().expect("tempdir");
    std::fs::create_dir_all(dir.path().join(".git").join("hooks")).expect("mkdir .git/hooks");

    let out = run_init_yes(&dir);
    assert!(out.status.success(), "init --yes must succeed");

    let hook_path = dir.path().join(".git").join("hooks").join("commit-msg");
    let content = std::fs::read_to_string(&hook_path).expect("read commit-msg");

    let has_bare = content
        .lines()
        .any(|line| line.trim_start().starts_with("8v hooks git on-commit-msg"));
    assert!(
        has_bare,
        "commit-msg hook must invoke bare `8v hooks git on-commit-msg`, not an absolute path; hook content:\n{content}"
    );

    let has_absolute = content.lines().filter(|l| !l.starts_with('#')).any(|line| {
        let cmd = line.split_whitespace().find(|t| *t != "exec").unwrap_or("");
        cmd.starts_with('/') || cmd.starts_with("'/") || cmd.starts_with("\"/")
    });
    assert!(
        !has_absolute,
        "commit-msg hook must not bake in an absolute binary path; hook content:\n{content}"
    );
}
