// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

pub(crate) mod check_human;
pub(crate) mod check_json;
pub(crate) mod check_plain;
pub(crate) mod fmt_human;
pub(crate) mod fmt_json;
pub(crate) mod fmt_plain;
pub mod output;
// Streaming renderers — per-event output for check commands.
pub mod stream_human;
pub mod stream_json;
pub mod stream_plain;
// Report types — structured data for each command.
pub mod build_report;
pub mod hooks_report;
pub mod ls_report;
pub mod read_report;
pub mod run_report;
pub mod search_report;
pub mod test_report;
pub mod upgrade_report;
pub mod write_report;

pub use output::Output;

// ─── Render types (rendering lives in o8v-core) ─────────────────────────────

/// Anything that flows to a consumer must be renderable.
/// Events, reports, and errors all implement this.
pub trait Renderable {
    /// Token-efficient text for AI agents. Default audience for MCP.
    fn render_plain(&self) -> Output;
    /// Structured JSON for machines, CI, programmatic use.
    fn render_json(&self) -> Output;
    /// Colored, aligned, symbol-rich for terminal users.
    /// Defaults to `render_plain`; override only when terminal output
    /// meaningfully differs (colors, alignment).
    fn render_human(&self) -> Output {
        self.render_plain()
    }
}

/// Who consumes the output. Determines which render method is called.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Audience {
    /// AI agent — token-efficient plain text
    Agent,
    /// Terminal user — colored, aligned, symbols
    Human,
    /// CI, scripts — structured JSON
    Machine,
}

/// Render a Renderable item for a given audience.
pub fn render(item: &impl Renderable, audience: Audience) -> Output {
    match audience {
        Audience::Agent => item.render_plain(),
        Audience::Human => item.render_human(),
        Audience::Machine => item.render_json(),
    }
}

/// No-op Renderable for commands with no progressive events.
impl Renderable for () {
    fn render_plain(&self) -> Output {
        Output::new(String::new())
    }
    fn render_json(&self) -> Output {
        Output::new(String::new())
    }
}

use crate::CheckReport;

impl Renderable for CheckReport {
    fn render_plain(&self) -> Output {
        check_plain::render_check_plain(self, &self.render_config)
    }
    fn render_json(&self) -> Output {
        check_json::render_check_json(self, &self.render_config)
    }
    fn render_human(&self) -> Output {
        check_human::render_check_human(self, &self.render_config)
    }
}

use crate::FmtReport;

impl Renderable for FmtReport {
    fn render_plain(&self) -> Output {
        fmt_plain::render_fmt_plain(self)
    }
    fn render_json(&self) -> Output {
        fmt_json::render_fmt_json(self)
    }
    fn render_human(&self) -> Output {
        fmt_human::render_fmt_human(self, &RenderConfig::default())
    }
}

// ─── Renderable impls for event types ────────────────────────────────────────

use crate::events;

impl Renderable for events::check::StreamCheckEvent {
    fn render_plain(&self) -> Output {
        match self {
            Self::ProjectStart { name, stack, .. } => Output::new(format!("{name} {stack}")),
            Self::ToolDone {
                name,
                outcome,
                duration_ms,
                diagnostic_count,
            } => Output::new(format!(
                "{name} {outcome} {duration_ms}ms {diagnostic_count} diagnostics"
            )),
            Self::DetectionError { message } => Output::new(format!("detection error: {message}")),
        }
    }
    fn render_json(&self) -> Output {
        let json = match self {
            Self::ProjectStart { name, stack, path } => {
                serde_json::json!({"event":"project_start","name":name,"stack":stack,"path":path})
            }
            Self::ToolDone {
                name,
                outcome,
                duration_ms,
                diagnostic_count,
            } => {
                serde_json::json!({"event":"tool_done","name":name,"outcome":outcome,"duration_ms":duration_ms,"diagnostic_count":diagnostic_count})
            }
            Self::DetectionError { message } => {
                serde_json::json!({"event":"detection_error","message":message})
            }
        };
        Output::new(match serde_json::to_string(&json) {
            Ok(s) => s,
            Err(e) => format!("{{\"error\":\"{}\"}}", e),
        })
    }
}

impl Renderable for events::fmt::FmtEvent {
    fn render_plain(&self) -> Output {
        match self {
            Self::Done {
                stack,
                tool,
                status,
                duration_ms,
            } => Output::new(format!("{stack}\t{status}\t{tool}\t{duration_ms}")),
        }
    }
    fn render_json(&self) -> Output {
        match self {
            Self::Done {
                stack,
                tool,
                status,
                duration_ms,
            } => {
                let json = serde_json::json!({"event":"fmt_done","stack":stack,"tool":tool,"status":status,"duration_ms":duration_ms});
                Output::new(match serde_json::to_string(&json) {
                    Ok(s) => s,
                    Err(e) => format!("{{\"error\":\"{}\"}}", e),
                })
            }
        }
    }
}

impl Renderable for events::test::TestEvent {
    fn render_plain(&self) -> Output {
        match self {
            Self::OutputLine { line, .. } => Output::new(line.clone()),
        }
    }
    fn render_json(&self) -> Output {
        match self {
            Self::OutputLine { line, stream } => {
                let s = match stream {
                    events::test::OutputStream::Stdout => "stdout",
                    events::test::OutputStream::Stderr => "stderr",
                };
                Output::new(
                    match serde_json::to_string(
                        &serde_json::json!({"event":"output_line","stream":s,"line":line}),
                    ) {
                        Ok(s) => s,
                        Err(e) => format!("{{\"error\":\"{}\"}}", e),
                    },
                )
            }
        }
    }
}

impl Renderable for events::build::BuildEvent {
    fn render_plain(&self) -> Output {
        match self {
            Self::OutputLine { line, .. } => Output::new(line.clone()),
        }
    }
    fn render_json(&self) -> Output {
        match self {
            Self::OutputLine { line, stream } => {
                let s = match stream {
                    events::test::OutputStream::Stdout => "stdout",
                    events::test::OutputStream::Stderr => "stderr",
                };
                Output::new(
                    match serde_json::to_string(
                        &serde_json::json!({"event":"output_line","stream":s,"line":line}),
                    ) {
                        Ok(s) => s,
                        Err(e) => format!("{{\"error\":\"{}\"}}", e),
                    },
                )
            }
        }
    }
}

impl Renderable for events::run::RunEvent {
    fn render_plain(&self) -> Output {
        match self {
            Self::OutputLine { line, .. } => Output::new(line.clone()),
        }
    }
    fn render_json(&self) -> Output {
        match self {
            Self::OutputLine { line, stream } => {
                let s = match stream {
                    events::test::OutputStream::Stdout => "stdout",
                    events::test::OutputStream::Stderr => "stderr",
                };
                Output::new(
                    match serde_json::to_string(
                        &serde_json::json!({"event":"output_line","stream":s,"line":line}),
                    ) {
                        Ok(s) => s,
                        Err(e) => format!("{{\"error\":\"{}\"}}", e),
                    },
                )
            }
        }
    }
}

impl Renderable for events::upgrade::UpgradeEvent {
    fn render_plain(&self) -> Output {
        match self {
            Self::Checking => Output::new("checking for updates".to_string()),
            Self::Downloading { percent } => Output::new(format!("downloading {}%", percent)),
            Self::Verifying => Output::new("verifying checksum".to_string()),
            Self::Replacing => Output::new("replacing binary".to_string()),
            Self::Installing { version } => Output::new(format!("installing {version}")),
            Self::AlreadyUpToDate { version } => {
                Output::new(format!("already up to date (v{version})"))
            }
            Self::Done { from, to } => Output::new(format!("upgraded {from} → {to}")),
        }
    }
    fn render_json(&self) -> Output {
        let json = match self {
            Self::Checking => serde_json::json!({"event":"checking"}),
            Self::Downloading { percent } => {
                serde_json::json!({"event":"downloading","percent":percent})
            }
            Self::Verifying => serde_json::json!({"event":"verifying"}),
            Self::Replacing => serde_json::json!({"event":"replacing"}),
            Self::Installing { version } => {
                serde_json::json!({"event":"installing","version":version})
            }
            Self::AlreadyUpToDate { version } => {
                serde_json::json!({"event":"already_up_to_date","version":version})
            }
            Self::Done { from, to } => serde_json::json!({"event":"done","from":from,"to":to}),
        };
        Output::new(match serde_json::to_string(&json) {
            Ok(s) => s,
            Err(e) => format!("{{\"error\":\"{}\"}}", e),
        })
    }
}

// ---------------------------------------------------------------------------
// Shared rendering types
// ---------------------------------------------------------------------------

/// Configuration for rendering a `CheckReport`.
#[derive(Debug)]
pub struct RenderConfig {
    /// Max lines of error detail per check. `None` = no limit.
    pub limit: Option<usize>,
    /// Show extra context (project path, timing).
    pub verbose: bool,
    /// Whether the output target supports color.
    pub color: bool,
    /// Page number (1-based). Default 1 (first page).
    pub page: usize,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            limit: Some(10),
            verbose: false,
            color: false,
            page: 1,
        }
    }
}

/// Computed summary of a `CheckReport` — shared by all renderers.
pub struct Summary {
    pub passed: u32,
    pub failed: u32,
    pub errors: u32,
    pub detection_errors: usize,
    pub total_duration: std::time::Duration,
    pub success: bool,
}

impl Summary {
    /// Compute summary from a report.
    #[must_use]
    pub fn from_report(report: &crate::CheckReport) -> Self {
        let mut passed = 0u32;
        let mut failed = 0u32;
        let mut errors = 0u32;
        let mut total_duration = std::time::Duration::ZERO;

        for result in report.results() {
            for entry in result.entries() {
                total_duration += entry.duration();
                match entry.outcome() {
                    crate::CheckOutcome::Passed { .. } => passed += 1,
                    crate::CheckOutcome::Failed { .. } => failed += 1,
                    // Error + any future non_exhaustive variants count as errors.
                    _ => errors += 1,
                }
            }
        }

        let det = report.detection_errors().len();
        // Single source of truth: CheckReport::is_ok() defines success.
        let success = report.is_ok();

        Self {
            passed,
            failed,
            errors,
            detection_errors: det,
            total_duration,
            success,
        }
    }
}

/// Sanitize a string for single-line terminal display.
///
/// Delegates to `crate::sanitize` — the single canonical implementation
/// that strips ANSI escape sequences and control characters (preserving tabs).
/// One function, one place, one behavior.
#[must_use]
pub fn sanitize_for_display(s: &str) -> String {
    crate::sanitize(s)
}

/// Re-export from `display_str` — single canonical definition.
pub use crate::DisplayStr;

#[cfg(test)]
mod tests {
    use super::*;

    // sanitize_for_display delegates to crate::sanitize — test the
    // delegation and key behaviors. Comprehensive ANSI tests live in o8v-check.

    #[test]
    fn sanitize_strips_ansi() {
        assert_eq!(sanitize_for_display("\x1b[31mred\x1b[0m"), "red");
    }

    #[test]
    fn sanitize_strips_control_chars() {
        assert_eq!(sanitize_for_display("a\x01b\x02c"), "abc");
        assert_eq!(sanitize_for_display("hello\x07world"), "helloworld");
        assert_eq!(sanitize_for_display("a\x7fb"), "ab");
    }

    #[test]
    fn sanitize_preserves_tabs_strips_newlines() {
        assert_eq!(sanitize_for_display("a\tb"), "a\tb");
        assert_eq!(sanitize_for_display("a\nb"), "ab");
        assert_eq!(sanitize_for_display("a\rb"), "ab");
    }

    #[test]
    fn sanitize_clean_string_unchanged() {
        assert_eq!(sanitize_for_display("hello world"), "hello world");
    }
}
