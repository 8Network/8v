// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! `DrillReport` — single-session drill-in (§4.2 / §4.6).

use super::helpers::{blind_spots_footer, fmt_bytes, fmt_duration_ms, fmt_time_hhmm, fmt_warning};
use crate::render::output::Output;
use crate::render::Renderable;
use crate::types::{TimestampMs, Warning};

/// Per-command p95 latency entry.
#[derive(Debug)]
pub struct PerCommandP95 {
    pub command: String,
    pub p95_ms: u64,
}

/// A cluster entry (retry or failure) for the drill view.
#[derive(Debug)]
pub struct ClusterEntry {
    pub kind: ClusterKind,
    pub command: String,
    pub argv_shape: String,
    pub count: usize,
    /// For failure clusters: path context extracted from argv_shape.
    pub path_hint: Option<String>,
}

#[derive(Debug)]
pub enum ClusterKind {
    Retry,
    Failure,
}

/// Top-command entry: command name + invocation count.
#[derive(Debug)]
pub struct TopCommand {
    pub command: String,
    pub count: usize,
}

/// Single-session drill-in report (§4.2 / §4.6).
#[derive(Debug)]
pub struct DrillReport {
    pub session_id: String,
    pub caller: String,
    pub agent: Option<String>,
    pub started_ms: Option<TimestampMs>,
    pub ended_ms: Option<TimestampMs>,
    pub project_path: Option<String>,
    pub command_count: usize,
    pub ok_count: usize,
    pub fail_count: usize,
    pub incomplete_count: usize,
    /// `None` when n < 5 — render as `-`.
    pub p50_ms: Option<u64>,
    /// `None` when n < 5 — render as `-`.
    pub p95_ms: Option<u64>,
    pub output_bytes_total: u64,
    pub per_command_p95: Vec<PerCommandP95>,
    pub top_commands: Vec<TopCommand>,
    pub clusters: Vec<ClusterEntry>,
    pub warnings: Vec<Warning>,
    /// Window used when building retry clusters (milliseconds). Default 30 000.
    pub retry_window_ms: u64,
}

impl Renderable for DrillReport {
    fn render_plain(&self) -> Output {
        let mut out = String::new();

        // Header line
        let when = match (self.started_ms, self.ended_ms) {
            (Some(s), Some(e)) => match e.checked_sub(s) {
                Some(dur) => format!(
                    "{}-{} ({})",
                    fmt_time_hhmm(s),
                    fmt_time_hhmm(e),
                    fmt_duration_ms(dur.as_millis())
                ),
                None => fmt_time_hhmm(s),
            },
            (Some(s), None) => fmt_time_hhmm(s),
            _ => "-".to_string(),
        };
        let agent_str = self.agent.as_deref().unwrap_or("-");
        out.push_str(&format!(
            "session {}   {}  {}  {}\n",
            self.session_id, when, agent_str, self.caller
        ));

        if let Some(p) = &self.project_path {
            out.push_str(&format!("project {}\n", p));
        }

        out.push_str("---\n");

        // Summary stats line
        let p50 = self
            .p50_ms
            .map(|v| format!("p50 {}ms", v))
            .unwrap_or_else(|| "p50 -".to_string());
        let p95 = self
            .p95_ms
            .map(|v| format!("p95 {}ms", v))
            .unwrap_or_else(|| "p95 -".to_string());
        out.push_str(&format!(
            "  {} commands   {} ok   {} fail   {}  {}   {} out\n",
            self.command_count,
            self.ok_count,
            self.fail_count,
            p50,
            p95,
            fmt_bytes(self.output_bytes_total),
        ));

        // Per-cmd p95
        if !self.per_command_p95.is_empty() {
            out.push('\n');
            out.push_str("per-cmd p95\n");
            for pc in &self.per_command_p95 {
                out.push_str(&format!(
                    "    {} {}\n",
                    pc.command,
                    fmt_duration_ms(pc.p95_ms)
                ));
            }
            out.push_str(
                "               (run '8v stats' for p50/p99, ok%, argv-shape breakdown)\n",
            );
        }

        // Top commands
        if !self.top_commands.is_empty() {
            out.push('\n');
            out.push_str("top commands\n");
            for tc in &self.top_commands {
                out.push_str(&format!("   {} {}\n", tc.command, tc.count));
            }
        }

        // Clusters
        let failures: Vec<&ClusterEntry> = self
            .clusters
            .iter()
            .filter(|c| matches!(c.kind, ClusterKind::Failure))
            .collect();
        let retries: Vec<&ClusterEntry> = self
            .clusters
            .iter()
            .filter(|c| matches!(c.kind, ClusterKind::Retry))
            .collect();

        if !failures.is_empty() {
            out.push_str("failures");
            for f in &failures {
                out.push_str(&format!("       {} x{}\n", f.command, f.count));
            }
        }
        if !retries.is_empty() {
            out.push_str("retries");
            for r in &retries {
                out.push_str(&format!(
                    "        {} {}   x{} in {}s\n",
                    r.command,
                    r.argv_shape,
                    r.count,
                    self.retry_window_ms / 1000
                ));
            }
        }

        if !self.warnings.is_empty() {
            out.push('\n');
            for w in &self.warnings {
                out.push_str(&format!("warning: {}\n", fmt_warning(w)));
            }
        }

        out.push('\n');
        out.push_str(blind_spots_footer(self.caller == "hook"));
        out.push('\n');

        Output::new(out)
    }

    fn render_json(&self) -> Output {
        // §4.6 stable JSON field names exactly.
        let per_cmd_p95: serde_json::Map<String, serde_json::Value> = self
            .per_command_p95
            .iter()
            .map(|pc| (pc.command.clone(), serde_json::Value::from(pc.p95_ms)))
            .collect();

        let top_cmds: Vec<serde_json::Value> = self
            .top_commands
            .iter()
            .map(|tc| serde_json::json!([tc.command, tc.count]))
            .collect();

        let clusters: Vec<serde_json::Value> = self
            .clusters
            .iter()
            .map(|c| {
                let kind = match c.kind {
                    ClusterKind::Retry => "retry",
                    ClusterKind::Failure => "failure",
                };
                serde_json::json!({
                    "kind": kind,
                    "command": c.command,
                    "argv_shape": c.argv_shape,
                    "count": c.count,
                    "path": c.path_hint,
                })
            })
            .collect();

        let agent_val = self.agent.as_ref().map(|a| {
            // Split "name version" if space present.
            let mut parts = a.splitn(2, ' ');
            let name = parts.next().unwrap_or(a.as_str());
            let version = parts.next().unwrap_or("");
            serde_json::json!({ "name": name, "version": version })
        });

        let json = serde_json::json!({
            "session_id": self.session_id,
            "caller": self.caller,
            "agent": agent_val,
            "started_ms": self.started_ms.map(|t| t.as_millis()),
            "ended_ms": self.ended_ms.map(|t| t.as_millis()),
            "commands": self.command_count,
            "ok": self.ok_count,
            "fail": self.fail_count,
            "incomplete": self.incomplete_count,
            "p50_ms": self.p50_ms,
            "p95_ms": self.p95_ms,
            "per_command_p95_ms": per_cmd_p95,
            "output_bytes_total": self.output_bytes_total,
            "top_commands": top_cmds,
            "clusters": clusters,
            "warnings": self.warnings,
        });

        let s = match serde_json::to_string_pretty(&json) {
            Ok(s) => s,
            Err(e) => format!("{{\"error\": \"serialization failed: {}\"}}", e),
        };
        Output::new(format!("{}\n", s))
    }
}
