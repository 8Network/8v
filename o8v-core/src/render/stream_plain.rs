// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Streaming plain text renderer — per-event token-efficient output.

use super::{Render, RenderConfig, Summary};
use crate::diagnostic::{Diagnostic, Location, ParseStatus};
use crate::{CheckOutcome, CheckReport};
use std::io::{self, Write};

pub struct Plain;

/// Public sub-methods for streaming. Same code path as batch.
impl Plain {
    /// Render one detection error.
    pub fn render_detection_error(
        &self,
        error: &o8v_project::DetectError,
        _config: &RenderConfig,
        w: &mut dyn Write,
    ) -> io::Result<()> {
        let msg = super::sanitize_for_display(&error.to_string());
        writeln!(w, "detection error: {msg}")
    }

    /// Render a project header.
    pub fn render_project_header(
        &self,
        name: &str,
        stack: o8v_project::Stack,
        path: &o8v_project::ProjectRoot,
        config: &RenderConfig,
        w: &mut dyn Write,
    ) -> io::Result<()> {
        let name = super::sanitize_for_display(name);
        if config.verbose {
            let path = super::sanitize_for_display(&path.to_string());
            writeln!(w, "{name} {stack} {path}")
        } else {
            writeln!(w, "{name} {stack}")
        }
    }

    /// Render one check entry: status + diagnostics.
    pub fn render_entry(
        &self,
        entry: &crate::CheckEntry,
        config: &RenderConfig,
        w: &mut dyn Write,
    ) -> io::Result<()> {
        let ms = entry.duration().as_millis();
        match entry.outcome() {
            CheckOutcome::Passed {
                parse_status,
                stdout_truncated,
                stderr_truncated,
                ..
            } => {
                let mut note = String::new();
                if *stdout_truncated || *stderr_truncated {
                    note.push_str(" (output truncated)");
                }
                if *parse_status == ParseStatus::Unparsed {
                    note.push_str(" (unparsed)");
                }
                writeln!(w, "{} passed {ms}ms{note}", entry.name())?;
            }
            CheckOutcome::Failed {
                diagnostics,
                raw_stdout,
                raw_stderr,
                parse_status,
                ..
            } => {
                writeln!(
                    w,
                    "{} failed {ms}ms {} diagnostics",
                    entry.name(),
                    diagnostics.len()
                )?;
                if !diagnostics.is_empty() {
                    write_diagnostics_plain(w, diagnostics, config.limit)?;
                } else if *parse_status == ParseStatus::Unparsed {
                    if !raw_stdout.is_empty() {
                        write_paginated(w, raw_stdout, config.limit)?;
                    }
                    if !raw_stderr.is_empty() {
                        write_paginated(w, raw_stderr, config.limit)?;
                    }
                }
            }
            CheckOutcome::Error {
                cause,
                raw_stdout,
                raw_stderr,
                ..
            } => {
                let cause = super::sanitize_for_display(cause);
                writeln!(w, "{} error {ms}ms {cause}", entry.name())?;
                if !raw_stdout.is_empty() {
                    write_paginated(w, raw_stdout, config.limit)?;
                }
                if !raw_stderr.is_empty() {
                    write_paginated(w, raw_stderr, config.limit)?;
                }
            }
            #[allow(unreachable_patterns)]
            other => {
                tracing::warn!(
                    "unknown CheckOutcome variant for '{}': {other:?}",
                    entry.name()
                );
                writeln!(w, "{} unknown {ms}ms", entry.name())?;
            }
        }
        Ok(())
    }

    /// Render the summary line.
    pub fn render_summary(
        &self,
        report: &CheckReport,
        _config: &RenderConfig,
        w: &mut dyn Write,
    ) -> io::Result<()> {
        let s = Summary::from_report(report);
        writeln!(w, "---")?;
        let result_label =
            if s.passed == 0 && s.failed == 0 && s.errors == 0 && s.detection_errors == 0 {
                "nothing"
            } else if s.success {
                "pass"
            } else {
                "fail"
            };
        write!(w, "result: {result_label}")?;
        write!(
            w,
            " {} passed {} failed {} errors",
            s.passed, s.failed, s.errors
        )?;
        if s.detection_errors > 0 {
            write!(w, " {} detection_errors", s.detection_errors)?;
        }
        writeln!(w, " {}ms", s.total_duration.as_millis())
    }
}

impl Render for Plain {
    fn render(
        &self,
        report: &CheckReport,
        config: &RenderConfig,
        w: &mut dyn Write,
    ) -> io::Result<()> {
        for err in report.detection_errors() {
            self.render_detection_error(err, config, w)?;
        }
        for result in report.results() {
            self.render_project_header(
                result.project_name(),
                result.stack(),
                result.project_path(),
                config,
                w,
            )?;
            for entry in result.entries() {
                self.render_entry(entry, config, w)?;
            }
        }
        self.render_summary(report, config, w)
    }
}

fn write_diagnostics_plain(
    w: &mut dyn Write,
    diagnostics: &[Diagnostic],
    limit: Option<usize>,
) -> io::Result<()> {
    let max_show = match limit {
        Some(0) | None => usize::MAX,
        Some(n) => n,
    };

    for (i, d) in diagnostics.iter().enumerate() {
        if i >= max_show {
            let remaining = diagnostics.len().saturating_sub(i);
            writeln!(w, "  … {remaining} more diagnostics")?;
            break;
        }

        let file = match &d.location {
            Location::File(f) | Location::Absolute(f) => f.as_str(),
        };

        let loc = d
            .span
            .as_ref()
            .map(|s| match (s.end_line, s.end_column) {
                (Some(el), Some(ec)) if el != s.line || ec != s.column => {
                    format!(":{}:{}-{}:{}", s.line, s.column, el, ec)
                }
                (Some(el), None) if el != s.line => {
                    format!(":{}:{}-{}", s.line, s.column, el)
                }
                _ => format!(":{}:{}", s.line, s.column),
            })
            .unwrap_or_default();

        let rule = d.rule.as_deref().unwrap_or("-");
        let sev = d.severity;

        writeln!(w, "  {file}{loc} {sev} {rule} {}", d.message.as_str())?;
    }

    Ok(())
}

const MAX_LINE_BYTES: usize = 4096;

fn write_paginated(w: &mut dyn Write, output: &str, limit: Option<usize>) -> io::Result<()> {
    let lines: Vec<&str> = output.lines().collect();
    let total = lines.len();

    let show = match limit {
        Some(0) | None => total,
        Some(n) => n.min(total),
    };

    for line in &lines[..show] {
        let sanitized = super::sanitize_for_display(line);
        if sanitized.len() > MAX_LINE_BYTES {
            let truncated = &sanitized[..sanitized.floor_char_boundary(MAX_LINE_BYTES)];
            writeln!(w, "  {truncated} … (line truncated)")?;
        } else {
            writeln!(w, "  {sanitized}")?;
        }
    }

    let remaining = total.saturating_sub(show);
    if remaining > 0 {
        writeln!(w, "  … {remaining} more lines")?;
    }

    Ok(())
}
