// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Pure function for constructing a normalized argv from a Claude tool name
//! and its JSON input, per design §3.3.
//!
//! Tool names are case-preserving on input (`"Bash"`, `"Read"`, etc.).
//! The first element of the returned argv is always the tool name lowercased.

use crate::hook::redact::redact_bash_command;

/// Build a normalized argv vector from a Claude tool name and its JSON input.
///
/// The first element is `tool_name` lowercased. Additional elements follow
/// the per-tool mapping documented in §3.3:
///
/// | Tool                                       | argv                            |
/// |--------------------------------------------|--------------------------------|
/// | `Bash`                                     | `["bash", redact(command)]`    |
/// | `Read` / `Edit` / `Write` / `NotebookEdit` | `[tool, "<path>"]`             |
/// | `Grep` / `Glob`                            | `[tool, "<str>"]`              |
/// | `Task`                                     | `[tool, "<str>"]`              |
/// | anything else                              | `[tool]`                       |
pub fn build_argv(tool_name: &str, tool_input: &serde_json::Value) -> Vec<String> {
    let tool_lower = tool_name.to_ascii_lowercase();

    match tool_lower.as_str() {
        "bash" => {
            let command = tool_input["command"].as_str().unwrap_or("");
            vec![tool_lower, redact_bash_command(command)]
        }
        "read" | "edit" | "write" | "notebookedit" => {
            vec![tool_lower, "<path>".to_string()]
        }
        "grep" | "glob" => {
            vec![tool_lower, "<str>".to_string()]
        }
        "task" => {
            vec![tool_lower, "<str>".to_string()]
        }
        _ => {
            vec![tool_lower]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn bash_produces_lowercased_tool_and_redacted_command() {
        let input = json!({ "command": "ls -la" });
        let argv = build_argv("Bash", &input);
        assert_eq!(argv[0], "bash");
        assert_eq!(argv[1], "ls -la");
        assert_eq!(argv.len(), 2);
    }

    #[test]
    fn bash_redacts_api_key_in_command() {
        let input =
            json!({ "command": "curl -H 'Authorization: Bearer sk-abcdefghijklmnopqrst12345'" });
        let argv = build_argv("Bash", &input);
        assert_eq!(argv[0], "bash");
        assert!(
            argv[1].contains("<secret>"),
            "api key must be redacted in argv: {:?}",
            argv[1]
        );
        assert!(!argv[1].contains("sk-abcdef"), "raw key must not appear");
    }

    #[test]
    fn bash_missing_command_field_produces_empty_string() {
        let input = json!({});
        let argv = build_argv("Bash", &input);
        assert_eq!(argv, vec!["bash", ""]);
    }

    #[test]
    fn read_produces_path_placeholder() {
        let input = json!({ "file_path": "/home/user/project/src/main.rs" });
        let argv = build_argv("Read", &input);
        assert_eq!(argv, vec!["read", "<path>"]);
    }

    #[test]
    fn edit_produces_path_placeholder() {
        let input = json!({ "file_path": "/tmp/file.rs" });
        let argv = build_argv("Edit", &input);
        assert_eq!(argv, vec!["edit", "<path>"]);
    }

    #[test]
    fn write_produces_path_placeholder() {
        let input = json!({ "file_path": "/tmp/output.txt" });
        let argv = build_argv("Write", &input);
        assert_eq!(argv, vec!["write", "<path>"]);
    }

    #[test]
    fn notebookedit_produces_path_placeholder() {
        let input = json!({ "notebook_path": "/tmp/nb.ipynb" });
        let argv = build_argv("NotebookEdit", &input);
        assert_eq!(argv, vec!["notebookedit", "<path>"]);
    }

    #[test]
    fn grep_produces_str_placeholder() {
        let input = json!({ "pattern": "fn main", "path": "/src" });
        let argv = build_argv("Grep", &input);
        assert_eq!(argv, vec!["grep", "<str>"]);
    }

    #[test]
    fn glob_produces_str_placeholder() {
        let input = json!({ "pattern": "**/*.rs" });
        let argv = build_argv("Glob", &input);
        assert_eq!(argv, vec!["glob", "<str>"]);
    }

    #[test]
    fn task_produces_str_placeholder() {
        let input = json!({ "description": "do something" });
        let argv = build_argv("Task", &input);
        assert_eq!(argv, vec!["task", "<str>"]);
    }

    #[test]
    fn unknown_tool_produces_only_lowercased_name() {
        let input = json!({ "some_field": "value" });
        let argv = build_argv("FutureTool", &input);
        assert_eq!(argv, vec!["futuretool"]);
    }

    #[test]
    fn tool_name_case_preserved_lowercased_in_output() {
        // Tool name "BASH" (all-caps) must produce "bash" as first element.
        let input = json!({ "command": "pwd" });
        let argv = build_argv("BASH", &input);
        assert_eq!(argv[0], "bash");
    }
}
