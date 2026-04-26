//! Output formatting utilities.

use std::time::Duration;

/// Marker appended when output is truncated.
pub const TRUNCATION_MARKER: &str = "... (truncated)";

/// Truncate a string to `max_bytes`, if needed.
///
/// Returns true if truncation occurred. Does NOT embed a marker in the string
/// — the `stdout_truncated`/`stderr_truncated` booleans on `ProcessResult`
/// are the authoritative signal. The string itself stays within the byte
/// contract: `s.len() <= max_bytes` after this call.
pub fn truncate_with_marker(s: &mut String, max_bytes: usize, stream_name: &str) -> bool {
    if s.len() > max_bytes {
        let original_len = s.len();
        s.truncate(s.floor_char_boundary(max_bytes));
        tracing::warn!(
            stream = stream_name,
            original_bytes = original_len,
            limit = max_bytes,
            "output truncated — diagnostics may be incomplete"
        );
        true
    } else {
        false
    }
}

/// Format a duration for human display.
///
/// Compact style: `123ms`, `1.5s`. Used in status lines, summaries, and
/// timeout error messages.
pub fn format_duration(d: Duration) -> String {
    let ms = d.as_millis();
    if ms < 1000 {
        format!("{ms}ms")
    } else {
        format!("{:.1}s", d.as_secs_f64())
    }
}
