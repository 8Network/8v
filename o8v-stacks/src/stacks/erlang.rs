//! Erlang stack — rebar3 compile, dialyzer, xref.

use crate::stack_tools::{BuildTool, FormatTool, StackTools, TestTool};
use crate::tool::EnrichedToolCheck;

/// Returns all tools for the Erlang stack.
pub fn tools() -> StackTools {
    StackTools {
        checks: vec![
            Box::new(EnrichedToolCheck {
                name: "rebar3 compile",
                program: "rebar3",
                args: &["compile"],
                stack: "erlang",
                parse_fn: crate::parse::rebar_compile::parse,
                env: &[],
                optional: false,
            }),
            Box::new(EnrichedToolCheck {
                name: "rebar3 dialyzer",
                program: "rebar3",
                args: &["dialyzer"],
                stack: "erlang",
                parse_fn: crate::parse::rebar_dialyzer::parse,
                env: &[],
                optional: false,
            }),
            Box::new(EnrichedToolCheck {
                name: "rebar3 xref",
                program: "rebar3",
                args: &["xref"],
                stack: "erlang",
                parse_fn: crate::parse::rebar_xref::parse,
                env: &[],
                optional: false,
            }),
        ],
        formatter: Some(FormatTool {
            program: "erlfmt",
            format_args: &["--write", "src/*.erl"],
            check_args: &["--check", "src/*.erl"],
            check_dirty_on_stdout: false,
            needs_node_resolution: false,
        }),
        test_runner: Some(TestTool {
            program: "rebar3",
            args: &["eunit"],
        }),
        build_tool: Some(BuildTool {
            program: "rebar3",
            args: &["compile"],
        }),
        error_extractor: None,
    }
}
