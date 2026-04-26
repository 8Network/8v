// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Shared formatting helpers used across all log report surfaces.

use crate::types::{TimestampMs, Warning};

/// Render a typed `Warning` into a single human-readable line.
/// Single source of truth for warning display — callers must never format
/// warnings themselves.
pub fn fmt_warning(w: &Warning) -> String {
    match w {
        Warning::CanonicalizeFailed { path, reason } => {
            format!("canonicalize_failed: {path} ({reason})")
        }
        Warning::DuplicateCompleted { run_id } => {
            format!("duplicate CommandCompleted run_id={run_id}: later dropped")
        }
        Warning::DuplicateStarted { run_id } => {
            format!("duplicate CommandStarted run_id={run_id}: first wins")
        }
        Warning::ReversedTimestamps {
            session,
            earlier,
            later,
        } => {
            format!(
                "reversed timestamps in session {}: started={} completed={}",
                session.as_str(),
                earlier.as_millis(),
                later.as_millis()
            )
        }
        Warning::EmptySessionId { at, reason } => {
            format!("empty session_id at ts={}: {reason}", at.as_millis())
        }
        Warning::FutureSince { since_ms, now_ms } => {
            format!("--since in the future: since={since_ms} now={now_ms}")
        }
        Warning::MalformedEventLine { line_no, reason } => {
            format!("line {line_no}: {reason}")
        }
        Warning::OrphanStarted { run_id } => {
            format!("orphan CommandStarted run_id={run_id}: no matching Completed")
        }
        Warning::OrphanCompleted { run_id } => {
            format!("orphan CommandCompleted run_id={run_id}: no matching Started")
        }
        Warning::NormalizerBasenameFallback { session, path } => {
            format!(
                "session {}: project_path unknown; basename fallback for {path}",
                session.as_str()
            )
        }
        Warning::PercentileOutOfRange { p } => {
            format!("percentile out of range: {p}")
        }
        Warning::FlagIgnoredForSession { flag } => {
            format!("{flag} ignored: session filter takes precedence")
        }
    }
}

pub const BLIND_SPOTS: &str =
    "blind spots: native Read/Edit/Bash invisible; write-success ≠ code-correct.";

/// Returns the appropriate blind-spots footer line.
///
/// When the filtered event set contains at least one hook-caller event the
/// "native Read/Edit/Bash invisible" clause is factually wrong — hook events
/// DO capture those tool calls.  Drop that clause and keep only the part that
/// is always true.
pub fn blind_spots_footer(has_hook_events: bool) -> &'static str {
    if has_hook_events {
        "blind spots: write-success ≠ code-correct."
    } else {
        BLIND_SPOTS
    }
}

/// Format a byte count as a human-readable string: KB / MB / B.
pub(crate) fn fmt_bytes(bytes: u64) -> String {
    if bytes >= 1_000_000 {
        format!("{:.1}MB", bytes as f64 / 1_000_000.0)
    } else if bytes >= 1_000 {
        format!("{:.1}KB", bytes as f64 / 1_000.0)
    } else {
        format!("{}B", bytes)
    }
}

/// Format a token count: K suffix when ≥ 1000.
pub(crate) fn fmt_tokens(tokens: u64) -> String {
    if tokens >= 1_000 {
        format!("{:.1}K", tokens as f64 / 1_000.0)
    } else {
        format!("{}", tokens)
    }
}

/// Format a duration in milliseconds as a human-readable string.
pub(crate) fn fmt_duration_ms(ms: u64) -> String {
    if ms >= 3_600_000 {
        format!("{}h{}m", ms / 3_600_000, (ms % 3_600_000) / 60_000)
    } else if ms >= 60_000 {
        format!("{}m", ms / 60_000)
    } else if ms >= 1_000 {
        format!("{:.1}s", ms as f64 / 1_000.0)
    } else {
        format!("{}ms", ms)
    }
}

/// Format a unix-ms timestamp as `HH:MM` (UTC, time-of-day only).
pub(crate) fn fmt_time_hhmm(ts: TimestampMs) -> String {
    let ms = ts.as_millis();
    if ms < 0 {
        return "??:??".to_string();
    }
    let secs = ms / 1000;
    let minutes_total = secs / 60;
    let hour = (minutes_total / 60) % 24;
    let minute = minutes_total % 60;
    format!("{:02}:{:02}", hour, minute)
}

/// Format a unix-ms timestamp as `YYYY-MM-DD HH:MM` (UTC).
///
/// Negative timestamps (pre-1970) are not meaningful 8v events; rendering
/// them through the naive subtraction loop below produced garbage
/// `1970-xx-xx` strings in the POC. Surface the problem instead.
pub(crate) fn fmt_timestamp(ts: TimestampMs) -> String {
    let ms = ts.as_millis();
    if ms < 0 {
        return "(invalid timestamp)".to_string();
    }
    let secs = ms / 1000;
    let minutes_total = secs / 60;
    let hour = (minutes_total / 60) % 24;
    let minute = minutes_total % 60;
    let days = secs / 86400;

    let mut y = 1970i32;
    let mut d = days as i32;
    loop {
        let days_in_year = if is_leap(y) { 366 } else { 365 };
        if d < days_in_year {
            break;
        }
        d -= days_in_year;
        y += 1;
    }
    let month_days: [i32; 12] = if is_leap(y) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut month = 1i32;
    for md in &month_days {
        if d < *md {
            break;
        }
        d -= md;
        month += 1;
    }
    let day = d + 1;
    format!("{:04}-{:02}-{:02} {:02}:{:02}", y, month, day, hour, minute)
}

pub(crate) fn is_leap(y: i32) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}
