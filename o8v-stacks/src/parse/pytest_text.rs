//! pytest text output parser — plain-text `pytest -q` / `pytest -v` output.
//!
//! Parses pytest's default text output (no plugins required). Extracts FAILED
//! lines and the associated traceback/assertion block that follows each failure.
//!
//! Output structure (simplified):
//! ```text
//! FAILED tests/test_foo.py::test_bar - AssertionError: assert 1 == 2
//! FAILED tests/test_foo.py::test_baz
//! ...
//! short test summary info
//! ```
//!
//! In verbose mode (`-v`) the FAILED lines appear in the summary section:
//! ```text
//! FAILED tests/test_foo.py::test_bar - AssertionError: ...
//! ```
//!
//! Collection errors look like:
//! ```text
//! ERROR collecting tests/test_broken.py
//! ```
//!
//! This parser is called from `python_extract` for `RunKind::Test`.
//! `RunKind::Build` returns an empty vec (Python has no build step).

use o8v_core::diagnostic::{Diagnostic, Location, Severity};
use o8v_core::display_str::DisplayStr;

/// Parse pytest plain-text output into diagnostics.
///
/// Returns a vec of [`Diagnostic`] — one per FAILED test or collection ERROR.
/// Returns an empty vec for clean output or unrecognised formats.
#[must_use]
pub fn parse(
    stdout: &str,
    _stderr: &str,
    _project_root: &std::path::Path,
    tool: &str,
    stack: &str,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    for line in stdout.lines() {
        let line = line.trim();

        // FAILED tests/test_foo.py::test_bar - AssertionError: ...
        // FAILED tests/test_foo.py::test_bar
        if let Some(rest) = line.strip_prefix("FAILED ") {
            let (test_id, message) = if let Some(idx) = rest.find(" - ") {
                let id = &rest[..idx];
                let msg = &rest[idx + 3..];
                (
                    id.trim(),
                    format!("{id} failed: {msg}", id = id.trim(), msg = msg.trim()),
                )
            } else {
                let id = rest.trim();
                (id, format!("{id} failed"))
            };

            // Extract file path from test id (everything before "::")
            let location = if let Some(sep) = test_id.find("::") {
                let file_part = &test_id[..sep];
                Location::File(file_part.to_string())
            } else {
                Location::Absolute(test_id.to_string())
            };

            diagnostics.push(Diagnostic {
                location,
                span: None,
                rule: Some(DisplayStr::from_untrusted("test")),
                severity: Severity::Error,
                raw_severity: Some("error".to_string()),
                message: DisplayStr::from_untrusted(&message),
                related: vec![],
                notes: vec![],
                suggestions: vec![],
                snippet: None,
                tool: tool.to_string(),
                stack: stack.to_string(),
            });
            continue;
        }

        // ERROR collecting tests/test_broken.py
        if let Some(rest) = line.strip_prefix("ERROR collecting ") {
            let file_path = rest.trim();
            let message = format!("collection error: {file_path}");
            diagnostics.push(Diagnostic {
                location: Location::File(file_path.to_string()),
                span: None,
                rule: Some(DisplayStr::from_untrusted("collection-error")),
                severity: Severity::Error,
                raw_severity: Some("error".to_string()),
                message: DisplayStr::from_untrusted(&message),
                related: vec![],
                notes: vec![],
                suggestions: vec![],
                snippet: None,
                tool: tool.to_string(),
                stack: stack.to_string(),
            });
        }
    }

    diagnostics
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn root() -> &'static Path {
        Path::new("/project")
    }

    /// Empty stdout → no diagnostics.
    #[test]
    fn empty_stdout() {
        let result = parse("", "", root(), "pytest", "python");
        assert!(result.is_empty());
    }

    /// One FAILED line with assertion message.
    #[test]
    fn one_failure_with_message() {
        let input = "FAILED tests/test_foo.py::test_bar - AssertionError: assert 1 == 2\n";
        let result = parse(input, "", root(), "pytest", "python");
        assert_eq!(result.len(), 1);
        let d = &result[0];
        assert_eq!(d.severity, Severity::Error);
        assert!(matches!(&d.location, Location::File(f) if f == "tests/test_foo.py"));
        assert!(d.message.to_string().contains("test_bar"));
        assert!(d.message.to_string().contains("AssertionError"));
        assert_eq!(
            d.rule.as_ref().map(|r| r.to_string()),
            Some("test".to_string())
        );
    }

    /// FAILED line without a message.
    #[test]
    fn one_failure_no_message() {
        let input = "FAILED tests/test_foo.py::test_baz\n";
        let result = parse(input, "", root(), "pytest", "python");
        assert_eq!(result.len(), 1);
        let d = &result[0];
        assert!(matches!(&d.location, Location::File(f) if f == "tests/test_foo.py"));
        assert!(d.message.to_string().contains("test_baz"));
    }

    /// Multiple failures → multiple diagnostics in order.
    #[test]
    fn many_failures() {
        let input = concat!(
            "FAILED tests/a.py::test_one - AssertionError: a\n",
            "FAILED tests/b.py::test_two - AssertionError: b\n",
            "FAILED tests/c.py::test_three\n",
        );
        let result = parse(input, "", root(), "pytest", "python");
        assert_eq!(result.len(), 3);
        assert!(matches!(&result[0].location, Location::File(f) if f == "tests/a.py"));
        assert!(matches!(&result[1].location, Location::File(f) if f == "tests/b.py"));
        assert!(matches!(&result[2].location, Location::File(f) if f == "tests/c.py"));
    }

    /// Collection error → diagnostic with collection-error rule.
    #[test]
    fn collection_error() {
        let input = "ERROR collecting tests/test_broken.py\n";
        let result = parse(input, "", root(), "pytest", "python");
        assert_eq!(result.len(), 1);
        let d = &result[0];
        assert!(matches!(&d.location, Location::File(f) if f == "tests/test_broken.py"));
        assert_eq!(
            d.rule.as_ref().map(|r| r.to_string()),
            Some("collection-error".to_string())
        );
        assert!(d.message.to_string().contains("collection error"));
    }

    /// Specific AssertionError message format → at least one diagnostic.
    #[test]
    fn failure_with_assert_expected() {
        let input = "FAILED test_foo.py::test_bar - AssertionError: expected 1\n";
        let result = parse(input, "", root(), "pytest", "python");
        assert!(
            !result.is_empty(),
            "expected at least 1 diagnostic for FAILED line"
        );
        assert!(result[0].message.to_string().contains("test_bar"));
        assert!(result[0].message.to_string().contains("expected 1"));
    }

    /// Noise lines (passed, warnings, summary header) are ignored.
    #[test]
    fn noise_lines_ignored() {
        let input = concat!(
            "collected 3 items\n",
            "tests/test_foo.py ..F  [100%]\n",
            "====== short test summary info ======\n",
            "1 passed, 1 failed in 0.42s\n",
        );
        let result = parse(input, "", root(), "pytest", "python");
        assert!(result.is_empty());
    }
}
