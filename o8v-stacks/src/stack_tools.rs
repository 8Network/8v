//! Stack tool definitions — formatters, test runners, and checks.
//!
//! Each stack defines everything it knows how to do:
//! - Checks for 8v check
//! - Formatter for 8v fmt
//! - Test runner for 8v test
//!
//! This is the single source of truth for a stack's capabilities.

use o8v_core::Check;

/// All tools that a stack provides: checks, formatter, test runner, build tool.
pub struct StackTools {
    /// All checks for 8v check (build + semantic + lint + format).
    pub checks: Vec<Box<dyn Check>>,
    /// Formatter for 8v fmt. None = no formatter for this stack.
    pub formatter: Option<FormatTool>,
    /// Test runner for 8v test. None = no test runner for this stack.
    pub test_runner: Option<TestTool>,
    /// Build tool for 8v build. None = no build step for this stack (e.g. Python).
    pub build_tool: Option<BuildTool>,
}

/// Configuration for a code formatter.
///
/// Formatters have two modes:
/// - Write mode: modify files in place using `format_args`
/// - Check mode: report if files need formatting using `check_args`
///
/// The exit code behavior varies by tool:
/// - Most: exit 0 on success, non-zero on failure
/// - gofmt: exits 0 even when dirty, uses stdout to list dirty files
///   (set `check_dirty_on_stdout: true` for these)
pub struct FormatTool {
    /// Program binary (e.g. "cargo", "prettier", "ruff", "gofmt").
    pub program: &'static str,
    /// Args for write mode (e.g. &["fmt", "--all"]).
    pub format_args: &'static [&'static str],
    /// Args for check mode (e.g. &["fmt", "--all", "--check"]).
    pub check_args: &'static [&'static str],
    /// If true, check mode uses stdout (not exit code) to detect dirty.
    /// Needed for gofmt -l which exits 0 but lists dirty files.
    pub check_dirty_on_stdout: bool,
    /// If true, resolve binary via find_node_bin walk-up.
    /// Used for npm/yarn/pnpm formatters that live in node_modules.
    pub needs_node_resolution: bool,
}

/// Configuration for a test runner.
///
/// Test runners are simple: a program and args. Exit code determines success.
pub struct TestTool {
    /// Program binary (e.g. "cargo", "npm", "pytest", "go").
    pub program: &'static str,
    /// Arguments to pass to the test runner.
    pub args: &'static [&'static str],
}

/// Configuration for a build tool.
///
/// Build tools compile or package the project. Exit code determines success.
/// The program is resolved from PATH — there is no node_modules resolution
/// for build tools (unlike checks, which use `find_node_bin`).
pub struct BuildTool {
    /// Program binary (e.g. "cargo", "go", "dotnet").
    pub program: &'static str,
    /// Arguments to pass to the build tool.
    pub args: &'static [&'static str],
}
