// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Provenance — capture-once metadata for a benchmark experiment.
//!
//! `Provenance::collect()` performs shell-outs (git, claude --version) and file
//! hashing. Call it once per experiment, then thread `&Provenance` through every
//! arm. Use `ProvenanceBuilder` in unit tests to avoid shell-outs.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;
use std::process::Command;

// ── Struct ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provenance {
    /// `cargo-pkg-version` of the testkit crate at compile time.
    pub pkg_version: String,
    /// `git rev-parse --short HEAD` with `-dirty` suffix when working tree is dirty.
    pub git_sha: String,
    /// Whether the working tree had uncommitted changes at capture time.
    pub git_dirty: bool,
    /// Output of `claude --version`, or `"unknown"` on failure.
    pub claude_cli_version: String,
    /// Model ID passed from `AgentResult`, or `None` when not available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// SHA-256 hex of the settings.json file content, or `"none"` when absent / not provided.
    pub settings_sha: String,
    /// SHA-256 hex of .mcp.json file content, or `"none"` when absent / not provided.
    pub mcp_config_sha: String,
    /// Unix timestamp (ms) when the experiment started.
    pub harness_started_at_unix_ms: i64,
    /// `"{OS}/{ARCH}"` — e.g. `"macos/aarch64"`.
    pub host_os: String,
    /// Rustc version captured at compile time via `option_env!("RUSTC_VERSION")`.
    pub rust_version: String,
}

impl Provenance {
    /// Collect provenance for a benchmark run. Shell-outs are attempted and
    /// failures are tolerated (field is set to `"unknown"` / `"none"`).
    pub fn collect(
        settings_path: Option<&Path>,
        mcp_config_path: Option<&Path>,
        model: Option<String>,
        started_at_ms: i64,
    ) -> Provenance {
        let (git_sha, git_dirty) = capture_git_info();
        Provenance {
            pkg_version: env!("CARGO_PKG_VERSION").to_string(),
            git_sha,
            git_dirty,
            claude_cli_version: capture_claude_version(),
            model,
            settings_sha: hash_file_or_none(settings_path),
            mcp_config_sha: hash_file_or_none(mcp_config_path),
            harness_started_at_unix_ms: started_at_ms,
            host_os: format!("{}/{}", std::env::consts::OS, std::env::consts::ARCH),
            rust_version: option_env!("RUSTC_VERSION")
                .unwrap_or("unknown")
                .to_string(),
        }
    }

    /// Deterministic correlation ID: SHA-256 hex of the JSON-serialised struct.
    pub fn provenance_id(&self) -> String {
        let empty = String::from("");
        let json_result = serde_json::to_string(self);
        let json = match json_result {
            Ok(s) => s,
            Err(_) => empty,
        };
        let mut hasher = Sha256::new();
        hasher.update(json.as_bytes());
        format!("{:x}", hasher.finalize())
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn capture_git_info() -> (String, bool) {
    let sha = match Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
    {
        Err(_) => "unknown".to_string(),
        Ok(o) => {
            if o.status.success() {
                String::from_utf8_lossy(&o.stdout).trim().to_string()
            } else {
                "unknown".to_string()
            }
        }
    };

    let dirty = match Command::new("git").args(["status", "--porcelain"]).output() {
        Err(_) => false,
        Ok(o) => !o.stdout.is_empty(),
    };

    let full_sha = if dirty && sha != "unknown" {
        format!("{}-dirty", sha)
    } else {
        sha
    };

    (full_sha, dirty)
}

fn capture_claude_version() -> String {
    match Command::new("claude").arg("--version").output() {
        Err(_) => "unknown".to_string(),
        Ok(o) => {
            if o.status.success() {
                String::from_utf8_lossy(&o.stdout).trim().to_string()
            } else {
                "unknown".to_string()
            }
        }
    }
}

fn hash_file_or_none(path: Option<&Path>) -> String {
    let path = match path {
        Some(p) => p,
        None => return "none".to_string(),
    };
    let content = match std::fs::read(path) {
        Ok(c) => c,
        Err(_) => return "none".to_string(),
    };
    let mut hasher = Sha256::new();
    hasher.update(&content);
    format!("{:x}", hasher.finalize())
}

// ── Builder (for tests) ──────────────────────────────────────────────────────

#[derive(Default)]
pub struct ProvenanceBuilder {
    pub pkg_version: Option<String>,
    pub git_sha: Option<String>,
    pub git_dirty: Option<bool>,
    pub claude_cli_version: Option<String>,
    pub model: Option<String>,
    pub settings_sha: Option<String>,
    pub mcp_config_sha: Option<String>,
    pub harness_started_at_unix_ms: Option<i64>,
    pub host_os: Option<String>,
    pub rust_version: Option<String>,
}

impl ProvenanceBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn pkg_version(mut self, v: impl Into<String>) -> Self {
        self.pkg_version = Some(v.into());
        self
    }
    pub fn git_sha(mut self, v: impl Into<String>) -> Self {
        self.git_sha = Some(v.into());
        self
    }
    pub fn git_dirty(mut self, v: bool) -> Self {
        self.git_dirty = Some(v);
        self
    }
    pub fn claude_cli_version(mut self, v: impl Into<String>) -> Self {
        self.claude_cli_version = Some(v.into());
        self
    }
    pub fn model(mut self, v: impl Into<String>) -> Self {
        self.model = Some(v.into());
        self
    }
    pub fn settings_sha(mut self, v: impl Into<String>) -> Self {
        self.settings_sha = Some(v.into());
        self
    }
    pub fn mcp_config_sha(mut self, v: impl Into<String>) -> Self {
        self.mcp_config_sha = Some(v.into());
        self
    }
    pub fn harness_started_at_unix_ms(mut self, v: i64) -> Self {
        self.harness_started_at_unix_ms = Some(v);
        self
    }
    pub fn host_os(mut self, v: impl Into<String>) -> Self {
        self.host_os = Some(v.into());
        self
    }
    pub fn rust_version(mut self, v: impl Into<String>) -> Self {
        self.rust_version = Some(v.into());
        self
    }

    pub fn build(self) -> Provenance {
        Provenance {
            pkg_version: self.pkg_version.unwrap_or_else(|| "0.0.0-test".to_string()),
            git_sha: self.git_sha.unwrap_or_else(|| "abc1234".to_string()),
            git_dirty: self.git_dirty.unwrap_or(false),
            claude_cli_version: self
                .claude_cli_version
                .unwrap_or_else(|| "1.0.0".to_string()),
            model: self.model,
            settings_sha: self.settings_sha.unwrap_or_else(|| "none".to_string()),
            mcp_config_sha: self.mcp_config_sha.unwrap_or_else(|| "none".to_string()),
            harness_started_at_unix_ms: self.harness_started_at_unix_ms.unwrap_or(0),
            host_os: self.host_os.unwrap_or_else(|| "test/test".to_string()),
            rust_version: self.rust_version.unwrap_or_else(|| "1.78.0".to_string()),
        }
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_provenance() -> Provenance {
        ProvenanceBuilder::new()
            .pkg_version("0.5.0")
            .git_sha("deadbeef")
            .git_dirty(true)
            .claude_cli_version("1.2.3")
            .model("claude-sonnet-4-6")
            .settings_sha("aabbcc")
            .mcp_config_sha("ddeeff")
            .harness_started_at_unix_ms(1_700_000_000_000)
            .host_os("macos/aarch64")
            .rust_version("1.78.0")
            .build()
    }

    #[test]
    fn round_trip_all_fields() {
        let p = sample_provenance();
        let json = serde_json::to_string(&p).expect("serialize");
        let p2: Provenance = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(p2.pkg_version, "0.5.0");
        assert_eq!(p2.git_sha, "deadbeef");
        assert!(p2.git_dirty);
        assert_eq!(p2.claude_cli_version, "1.2.3");
        assert_eq!(p2.model.as_deref(), Some("claude-sonnet-4-6"));
        assert_eq!(p2.settings_sha, "aabbcc");
        assert_eq!(p2.mcp_config_sha, "ddeeff");
        assert_eq!(p2.harness_started_at_unix_ms, 1_700_000_000_000);
        assert_eq!(p2.host_os, "macos/aarch64");
        assert_eq!(p2.rust_version, "1.78.0");
    }

    #[test]
    fn provenance_id_is_deterministic() {
        let p = sample_provenance();
        let id1 = p.provenance_id();
        let id2 = p.provenance_id();
        assert_eq!(id1, id2);
        assert_eq!(id1.len(), 64, "SHA-256 hex is 64 chars");
    }

    #[test]
    fn provenance_id_changes_with_fields() {
        let p1 = sample_provenance();
        let p2 = ProvenanceBuilder::new().git_sha("cafebabe").build();
        assert_ne!(p1.provenance_id(), p2.provenance_id());
    }

    #[test]
    fn model_none_omitted_from_json() {
        let p = ProvenanceBuilder::new().build();
        let json = serde_json::to_string(&p).expect("serialize");
        assert!(!json.contains("\"model\""), "model:None must be omitted");
    }
}
