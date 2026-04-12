// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Streaming human renderer — per-event colored output for terminals.

use super::{Render, RenderConfig, Summary};
use crate::diagnostic::{Diagnostic, Location, ParseStatus, Severity};
use crate::{CheckOutcome, CheckReport};
use std::io::{self, Write};

pub struct Human;

// ANSI codes
const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const RED: &str = "\x1b[31m";
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const RESET: &str = "\x1b[0m";

/// Public sub-methods for streaming. The CLI calls these directly.
impl Human {
    /// Render one detection error.
    pub fn render_detection_error(
        &self,
        error: &o8v_project::DetectError,
        config: &RenderConfig,
        w: &mut dyn Write,
    ) -> io::Result<()> {
        let msg = super::sanitize_for_display(&error.to_string());
        writeln!(
            w,
            "  {}error:{} {msg}",
            color(config.color, RED),
            color(config.color, RESET)
        )
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
        let c = config.color;
        let name = super::sanitize_for_display(name);
        writeln!(w)?;
        if config.verbose {
            let path = super::sanitize_for_display(&path.to_string());
            writeln!(
                w,
                "  {}{}{} {}{}{} ({})",
                color(c, BOLD),
                name,
                color(c, RESET),
                color(c, DIM),
                stack,
                color(c, RESET),
                path
            )?;
        } else {
            writeln!(
                w,
                "  {}{}{} {}{}{}",
                color(c, BOLD),
                name,
                color(c, RESET),
                color(c, DIM),
                stack,
                color(c, RESET)
            )?;
        }
        writeln!(w)
    }

    /// Render one check entry: status line + diagnostics/error detail.
    pub fn render_entry(
        &self,
        entry: &crate::CheckEntry,
        config: &RenderConfig,
        w: &mut dyn Write,
    ) -> io::Result<()> {
        let c = config.color;
        write_check_line(w, entry, 0, c)?;
        write_entry_detail(w, entry, config, c)
    }

    /// Render the summary line.
    pub fn render_summary(
        &self,
        report: &CheckReport,
        config: &RenderConfig,
        w: &mut dyn Write,
    ) -> io::Result<()> {
        writeln!(w)?;
        write_summary(w, report, config.color)
    }
}

impl Render for Human {
    fn render(
        &self,
        report: &CheckReport,
        config: &RenderConfig,
        w: &mut dyn Write,
    ) -> io::Result<()> {
        let c = config.color;

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

            let max_name = result
                .entries()
                .iter()
                .map(|e| e.name().len())
                .max()
                .unwrap_or(0);

            for entry in result.entries() {
                write_check_line(w, entry, max_name, c)?;
            }
            for entry in result.entries() {
                write_entry_detail(w, entry, config, c)?;
            }
        }

        self.render_summary(report, config, w)
    }
}

fn write_entry_detail(
    w: &mut dyn Write,
    entry: &crate::CheckEntry,
    config: &RenderConfig,
    c: bool,
) -> io::Result<()> {
    match entry.outcome() {
        CheckOutcome::Failed {
            diagnostics,
            raw_stdout,
            raw_stderr,
            parse_status,
            ..
        } => {
            if !diagnostics.is_empty() {
                writeln!(w)?;
                write_diagnostics(w, diagnostics, config.limit, c)?;
            } else if *parse_status == ParseStatus::Unparsed {
                let mut combined = String::new();
                if !raw_stdout.is_empty() {
                    combined.push_str(raw_stdout);
                }
                if !raw_stderr.is_empty() {
                    if !combined.is_empty() {
                        combined.push('\n');
                    }
                    combined.push_str(raw_stderr);
                }
                if !combined.is_empty() {
                    writeln!(w)?;
                    write_error_box(w, entry.name(), &combined, config.limit, c)?;
                }
            }
        }
        CheckOutcome::Error {
            raw_stdout,
            raw_stderr,
            ..
        } => {
            let mut output = String::new();
            if !raw_stdout.is_empty() {
                output.push_str(raw_stdout);
            }
            if !raw_stderr.is_empty() {
                if !output.is_empty() {
                    output.push('\n');
                }
                output.push_str(raw_stderr);
            }
            if !output.is_empty() {
                writeln!(w)?;
                write_error_box(w, entry.name(), &output, config.limit, c)?;
            }
        }
        CheckOutcome::Passed { .. } => {}
        #[allow(unreachable_patterns)]
        other => {
            tracing::warn!(
                "unknown CheckOutcome variant for '{}': {other:?}",
                entry.name()
            );
        }
    }
    Ok(())
}

fn write_check_line(
    w: &mut dyn Write,
    entry: &crate::CheckEntry,
    max_name: usize,
    c: bool,
) -> io::Result<()> {
    let padded = format!("{:width$}", entry.name(), width = max_name);
    let dur = format_duration(entry.duration());

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
            writeln!(
                w,
                "    {padded}  {}✓{}   {}{dur}{}{}{}{}",
                color(c, GREEN),
                color(c, RESET),
                color(c, DIM),
                color(c, RESET),
                color(c, DIM),
                note,
                color(c, RESET),
            )
        }
        CheckOutcome::Failed { .. } => {
            writeln!(
                w,
                "    {padded}  {}✗{}   {}{dur}{}",
                color(c, RED),
                color(c, RESET),
                color(c, DIM),
                color(c, RESET)
            )
        }
        CheckOutcome::Error { cause, .. } => {
            let cause = super::sanitize_for_display(cause);
            writeln!(
                w,
                "    {padded}  {}!{}   {}{cause}{}",
                color(c, YELLOW),
                color(c, RESET),
                color(c, DIM),
                color(c, RESET)
            )
        }
        #[allow(unreachable_patterns)]
        other => {
            tracing::warn!(
                "unknown CheckOutcome variant for '{}': {other:?}",
                entry.name()
            );
            writeln!(
                w,
                "    {padded}  {}?{}   unknown",
                color(c, YELLOW),
                color(c, RESET)
            )
        }
    }
}

fn write_diagnostics(
    w: &mut dyn Write,
    diagnostics: &[Diagnostic],
    limit: Option<usize>,
    c: bool,
) -> io::Result<()> {
    let mut by_file: std::collections::BTreeMap<&str, Vec<&Diagnostic>> =
        std::collections::BTreeMap::new();
    for d in diagnostics {
        let file = match &d.location {
            Location::File(f) | Location::Absolute(f) => f.as_str(),
        };
        by_file.entry(file).or_default().push(d);
    }

    let max_show = match limit {
        Some(0) | None => usize::MAX,
        Some(n) => n,
    };
    let mut shown = 0usize;

    for (file, diags) in &by_file {
        if shown >= max_show {
            let remaining = diagnostics.len().saturating_sub(shown);
            if remaining > 0 {
                writeln!(
                    w,
                    "    {}… {remaining} more diagnostics{}",
                    color(c, DIM),
                    color(c, RESET)
                )?;
            }
            return Ok(());
        }

        writeln!(w, "    {}{file}{}", color(c, BOLD), color(c, RESET))?;

        for d in diags {
            if shown >= max_show {
                let remaining = diagnostics.len().saturating_sub(shown);
                if remaining > 0 {
                    writeln!(
                        w,
                        "    {}… {remaining} more diagnostics{}",
                        color(c, DIM),
                        color(c, RESET)
                    )?;
                }
                return Ok(());
            }

            let loc = d
                .span
                .as_ref()
                .map_or(String::new(), |s| match (s.end_line, s.end_column) {
                    (Some(el), Some(ec)) if el != s.line || ec != s.column => {
                        format!("{}:{}-{}:{}", s.line, s.column, el, ec)
                    }
                    (Some(el), None) if el != s.line => {
                        format!("{}:{}-{}", s.line, s.column, el)
                    }
                    _ => format!("{}:{}", s.line, s.column),
                });

            let sev_color = match d.severity {
                Severity::Error => RED,
                Severity::Warning => YELLOW,
                _ => DIM,
            };

            let rule = d.rule.as_deref().unwrap_or("");

            writeln!(
                w,
                "      {}{loc:>8}{}  {}{:7}{}  {}  {}{}{}",
                color(c, DIM),
                color(c, RESET),
                color(c, sev_color),
                d.severity,
                color(c, RESET),
                d.message.as_str(),
                color(c, DIM),
                rule,
                color(c, RESET),
            )?;

            shown += 1;
        }

        writeln!(w)?;
    }

    Ok(())
}

fn write_error_box(
    w: &mut dyn Write,
    name: &str,
    output: &str,
    limit: Option<usize>,
    c: bool,
) -> io::Result<()> {
    let lines: Vec<&str> = output.lines().collect();
    let total = lines.len();
    let show = match limit {
        Some(0) | None => total,
        Some(n) => n.min(total),
    };

    writeln!(w, "    {}┌ {name}{}", color(c, RED), color(c, RESET))?;

    const MAX_LINE_BYTES: usize = 4096;
    for line in &lines[..show] {
        let clean = super::sanitize_for_display(line);
        if clean.len() > MAX_LINE_BYTES {
            let truncated = &clean[..clean.floor_char_boundary(MAX_LINE_BYTES)];
            writeln!(
                w,
                "    {}│{} {truncated} … (line truncated)",
                color(c, RED),
                color(c, RESET)
            )?;
        } else {
            writeln!(w, "    {}│{} {clean}", color(c, RED), color(c, RESET))?;
        }
    }

    let remaining = total.saturating_sub(show);
    if remaining > 0 {
        writeln!(
            w,
            "    {}│{} {}{remaining} more lines{}",
            color(c, RED),
            color(c, RESET),
            color(c, DIM),
            color(c, RESET)
        )?;
    }

    writeln!(w, "    {}└{}", color(c, RED), color(c, RESET))?;

    Ok(())
}

fn write_summary(w: &mut dyn Write, report: &CheckReport, c: bool) -> io::Result<()> {
    let s = Summary::from_report(report);
    let total = s.passed + s.failed + s.errors;
    let passed = s.passed;
    let failed = s.failed;
    let errors = s.errors;
    let det = s.detection_errors;
    let total_duration = s.total_duration;

    if total == 0 && det == 0 {
        write!(
            w,
            "  {}no projects detected{}",
            color(c, YELLOW),
            color(c, RESET)
        )?;
    } else if failed == 0 && errors == 0 && det == 0 && total > 0 {
        write!(w, "  {}✓ all passed{}", color(c, GREEN), color(c, RESET))?;
    } else {
        write!(w, " ")?;
        if failed > 0 {
            write!(w, " {}{failed} failed{}", color(c, RED), color(c, RESET))?;
        }
        if passed > 0 {
            write!(w, "  {}{passed} passed{}", color(c, GREEN), color(c, RESET))?;
        }
        if errors > 0 {
            write!(
                w,
                "  {}{errors} error{}{}",
                color(c, YELLOW),
                if errors == 1 { "" } else { "s" },
                color(c, RESET)
            )?;
        }
        if det > 0 {
            write!(
                w,
                "  {}{det} detection error{}{}",
                color(c, YELLOW),
                if det == 1 { "" } else { "s" },
                color(c, RESET)
            )?;
        }
    }

    writeln!(
        w,
        "  {}{}{}",
        color(c, DIM),
        format_duration(total_duration),
        color(c, RESET)
    )?;

    Ok(())
}

const fn color(enabled: bool, code: &str) -> &str {
    if enabled {
        code
    } else {
        ""
    }
}

fn format_duration(d: std::time::Duration) -> String {
    let ms = d.as_millis();
    if ms < 1000 {
        format!("{ms}ms")
    } else {
        format!("{:.1}s", d.as_secs_f64())
    }
}
