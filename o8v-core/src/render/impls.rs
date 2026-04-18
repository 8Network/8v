// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! `impl Renderable` blocks for report types and event types defined in other crates.

use super::config::RenderConfig;
use super::output::Output;
use super::renderable::Renderable;
use super::{check_human, check_json, check_plain, fmt_human, fmt_json, fmt_plain};
use crate::{events, CheckReport, FmtReport};

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
