// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

use super::output::Output;
use serde::{Deserialize, Serialize};

/// What kind of write operation was performed, with the content needed to render
/// the same diff output that `write_to_file` produces.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum WriteOperation {
    /// A line or range was replaced.
    /// `old_lines` — lines that were removed.
    /// `new_content` — the single replacement line.
    Replace {
        old_lines: Vec<String>,
        new_content: String,
    },
    /// A line was inserted before an anchor line.
    Insert { content: String },
    /// Lines were deleted.
    Delete { deleted_lines: Vec<String> },
    /// A new file was created.
    Create { line_count: usize },
    /// Content was appended to an existing file.
    Append,
    /// Find-and-replace was performed.
    FindReplace { count: usize },
    /// Find-and-replace found no matches.
    FindReplaceNoMatch { find: String },
}

/// Report of a completed write operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteReport {
    pub path: String,
    pub operation: WriteOperation,
}

impl super::Renderable for WriteReport {
    fn render_plain(&self) -> Output {
        let text = match &self.operation {
            WriteOperation::Replace {
                old_lines,
                new_content,
            } => {
                let mut out = format!("{}  replaced\n", self.path);
                for old in old_lines {
                    out.push_str(&format!("  - {old}\n"));
                }
                out.push_str(&format!("  + {new_content}\n"));
                out
            }
            WriteOperation::Insert { content } => {
                format!("{}  inserted\n  + {content}\n", self.path)
            }
            WriteOperation::Delete { deleted_lines } => {
                let count = deleted_lines.len();
                let mut out = format!("{}  deleted ({count} lines)\n", self.path);
                for line in deleted_lines {
                    out.push_str(&format!("  - {line}\n"));
                }
                out
            }
            WriteOperation::Create { line_count } => {
                format!("{}  created ({line_count} lines)\n", self.path)
            }
            WriteOperation::Append => {
                format!("{}  appended\n", self.path)
            }
            WriteOperation::FindReplace { count } => {
                let s = if *count == 1 { "" } else { "s" };
                format!("{}  replaced ({count} occurrence{s})\n", self.path)
            }
            WriteOperation::FindReplaceNoMatch { find } => {
                format!("{}  no matches found for: {find}\n", self.path)
            }
        };
        Output::new(text)
    }

    fn render_json(&self) -> Output {
        let json = match serde_json::to_string(self) {
            Ok(s) => s,
            Err(e) => format!("{{\"error\": \"serialization failed: {e}\"}}"),
        };
        Output::new(json)
    }

    fn render_human(&self) -> Output {
        self.render_plain()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::Renderable;

    #[test]
    fn test_render_plain_replace_single_line() {
        let report = WriteReport {
            path: "src/main.rs".to_string(),
            operation: WriteOperation::Replace {
                old_lines: vec!["old line".to_string()],
                new_content: "new line".to_string(),
            },
        };

        let text = report.render_plain().to_string();
        assert_eq!(text, "src/main.rs  replaced\n  - old line\n  + new line\n");
    }

    #[test]
    fn test_render_plain_replace_range() {
        let report = WriteReport {
            path: "file.txt".to_string(),
            operation: WriteOperation::Replace {
                old_lines: vec!["line a".to_string(), "line b".to_string()],
                new_content: "replacement".to_string(),
            },
        };

        let text = report.render_plain().to_string();
        assert_eq!(
            text,
            "file.txt  replaced\n  - line a\n  - line b\n  + replacement\n"
        );
    }

    #[test]
    fn test_render_plain_insert() {
        let report = WriteReport {
            path: "test.rs".to_string(),
            operation: WriteOperation::Insert {
                content: "inserted line".to_string(),
            },
        };

        let text = report.render_plain().to_string();
        assert_eq!(text, "test.rs  inserted\n  + inserted line\n");
    }

    #[test]
    fn test_render_plain_delete() {
        let report = WriteReport {
            path: "config.json".to_string(),
            operation: WriteOperation::Delete {
                deleted_lines: vec![
                    "line 1".to_string(),
                    "line 2".to_string(),
                    "line 3".to_string(),
                ],
            },
        };

        let text = report.render_plain().to_string();
        assert_eq!(
            text,
            "config.json  deleted (3 lines)\n  - line 1\n  - line 2\n  - line 3\n"
        );
    }

    #[test]
    fn test_render_plain_create() {
        let report = WriteReport {
            path: "new_file.txt".to_string(),
            operation: WriteOperation::Create { line_count: 5 },
        };

        let text = report.render_plain().to_string();
        assert_eq!(text, "new_file.txt  created (5 lines)\n");
    }

    #[test]
    fn test_render_plain_append() {
        let report = WriteReport {
            path: "log.txt".to_string(),
            operation: WriteOperation::Append,
        };

        let text = report.render_plain().to_string();
        assert_eq!(text, "log.txt  appended\n");
    }

    #[test]
    fn test_render_plain_find_replace_single() {
        let report = WriteReport {
            path: "data.txt".to_string(),
            operation: WriteOperation::FindReplace { count: 1 },
        };

        let text = report.render_plain().to_string();
        assert_eq!(text, "data.txt  replaced (1 occurrence)\n");
    }

    #[test]
    fn test_render_plain_find_replace_multiple() {
        let report = WriteReport {
            path: "data.txt".to_string(),
            operation: WriteOperation::FindReplace { count: 3 },
        };

        let text = report.render_plain().to_string();
        assert_eq!(text, "data.txt  replaced (3 occurrences)\n");
    }

    #[test]
    fn test_render_plain_find_replace_no_match() {
        let report = WriteReport {
            path: "data.txt".to_string(),
            operation: WriteOperation::FindReplaceNoMatch {
                find: "missing".to_string(),
            },
        };

        let text = report.render_plain().to_string();
        assert_eq!(text, "data.txt  no matches found for: missing\n");
    }

    #[test]
    fn test_render_json_replace() {
        let report = WriteReport {
            path: "lib.rs".to_string(),
            operation: WriteOperation::Replace {
                old_lines: vec!["old".to_string()],
                new_content: "new".to_string(),
            },
        };

        let text = report.render_json().to_string();
        assert!(text.contains("\"path\""));
        assert!(text.contains("\"operation\""));
        assert!(text.contains("\"replace\""));
        assert!(text.contains("\"old_lines\""));
        assert!(text.contains("\"new_content\""));
    }

    #[test]
    fn test_render_json_find_replace() {
        let report = WriteReport {
            path: "data.txt".to_string(),
            operation: WriteOperation::FindReplace { count: 7 },
        };

        let text = report.render_json().to_string();
        assert!(text.contains("\"find_replace\""));
        assert!(text.contains("\"count\""));
    }

    #[test]
    fn test_render_human_matches_plain() {
        let report = WriteReport {
            path: "module.rs".to_string(),
            operation: WriteOperation::Insert {
                content: "fn foo() {}".to_string(),
            },
        };

        assert_eq!(
            report.render_plain().to_string(),
            report.render_human().to_string()
        );
    }
}
