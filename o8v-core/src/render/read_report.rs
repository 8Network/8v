// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

use super::output::Output;
use serde::{Deserialize, Serialize};

/// What kind of read was performed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReadReport {
    /// Symbol map — functions, structs, etc.
    Symbols {
        path: String,
        total_lines: usize,
        symbols: Vec<SymbolEntry>,
    },
    /// Line range.
    Range {
        path: String,
        start: usize,
        end: usize,
        total_lines: usize,
        lines: Vec<LineEntry>,
    },
    /// Full file content.
    Full {
        path: String,
        total_lines: usize,
        lines: Vec<LineEntry>,
    },
    /// Multiple files read in one call.
    Multi { entries: Vec<MultiEntry> },
}

/// One entry in a multi-file read — either success or error.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiEntry {
    /// The path label (as given by the user, including any range suffix).
    pub label: String,
    /// The result for this path.
    pub result: MultiResult,
}

/// Result for a single file in a multi-file read.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status")]
pub enum MultiResult {
    /// Successfully read — contains the full sub-report.
    Ok { report: Box<ReadReport> },
    /// Failed — contains the error message.
    Err { message: String },
}

/// A single symbol extracted from a file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolEntry {
    pub name: String,
    pub kind: String,
    pub line: usize,
    pub signature: String,
}

/// A single line of file content.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineEntry {
    pub line: usize,
    pub text: String,
}

impl super::Renderable for ReadReport {
    fn render_plain(&self) -> Output {
        match self {
            ReadReport::Symbols {
                path,
                total_lines,
                symbols,
            } => {
                let mut output = format!("{path} ({total_lines} lines)\n");
                if symbols.is_empty() {
                    output.push_str("\n  (no symbols found)\n");
                } else {
                    output.push('\n');
                    for sym in symbols {
                        output.push_str(&format!("{:>4}  {}\n", sym.line, sym.signature));
                    }
                }
                Output::new(output)
            }
            ReadReport::Range {
                path,
                start,
                end,
                total_lines,
                lines,
            } => {
                let mut output = format!("{path}:{start}-{end} (of {total_lines} lines)\n\n");
                for entry in lines {
                    output.push_str(&format!("{:>4}  {}\n", entry.line, entry.text));
                }
                Output::new(output)
            }
            ReadReport::Full {
                path,
                total_lines,
                lines,
            } => {
                let mut output = format!("{path} ({total_lines} lines)\n\n");
                for entry in lines {
                    output.push_str(&format!("{:>4}  {}\n", entry.line, entry.text));
                }
                Output::new(output)
            }
            ReadReport::Multi { entries } => {
                let mut output = String::new();
                for (i, entry) in entries.iter().enumerate() {
                    if i > 0 {
                        output.push('\n');
                    }
                    output.push_str(&format!("=== {} ===\n", entry.label));
                    match &entry.result {
                        MultiResult::Ok { report } => {
                            // render the sub-report but strip its trailing newline if present,
                            // then re-add consistently.
                            let sub = report.render_plain();
                            output.push_str(sub.as_str());
                        }
                        MultiResult::Err { message } => {
                            output.push_str(&format!("error: {message}\n"));
                        }
                    }
                }
                Output::new(output)
            }
        }
    }

    fn render_json(&self) -> Output {
        let json = match serde_json::to_string(self) {
            Ok(s) => s,
            Err(e) => format!("{{\"error\": \"serialization failed: {e}\"}}"),
        };
        Output::new(json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::Renderable;

    #[test]
    fn test_render_plain_symbols() {
        let report = ReadReport::Symbols {
            path: "src/main.rs".to_string(),
            total_lines: 20,
            symbols: vec![
                SymbolEntry {
                    name: "main".to_string(),
                    kind: "function".to_string(),
                    line: 10,
                    signature: "pub fn main()".to_string(),
                },
                SymbolEntry {
                    name: "MyStruct".to_string(),
                    kind: "struct".to_string(),
                    line: 5,
                    signature: "struct MyStruct {".to_string(),
                },
            ],
        };

        let output = report.render_plain();
        let text = output.to_string();
        assert!(text.contains("src/main.rs (20 lines)"));
        assert!(text.contains("pub fn main()"));
        assert!(text.contains("struct MyStruct {"));
        // Line numbers are right-aligned in 4 chars
        assert!(text.contains("  10  pub fn main()"));
        assert!(text.contains("   5  struct MyStruct {"));
    }

    #[test]
    fn test_render_plain_symbols_empty() {
        let report = ReadReport::Symbols {
            path: "empty.txt".to_string(),
            total_lines: 10,
            symbols: vec![],
        };

        let output = report.render_plain();
        let text = output.to_string();
        assert!(text.contains("empty.txt (10 lines)"));
        assert!(text.contains("no symbols found"));
    }

    #[test]
    fn test_render_json_symbols() {
        let report = ReadReport::Symbols {
            path: "test.rs".to_string(),
            total_lines: 5,
            symbols: vec![SymbolEntry {
                name: "foo".to_string(),
                kind: "function".to_string(),
                line: 1,
                signature: "fn foo()".to_string(),
            }],
        };

        let output = report.render_json();
        let text = output.to_string();
        assert!(text.contains("\"path\""));
        assert!(text.contains("\"symbols\""));
        assert!(text.contains("\"foo\""));
    }

    #[test]
    fn test_render_plain_range() {
        let report = ReadReport::Range {
            path: "file.txt".to_string(),
            start: 10,
            end: 12,
            total_lines: 20,
            lines: vec![
                LineEntry {
                    line: 10,
                    text: "first line".to_string(),
                },
                LineEntry {
                    line: 11,
                    text: "second line".to_string(),
                },
                LineEntry {
                    line: 12,
                    text: "third line".to_string(),
                },
            ],
        };

        let output = report.render_plain();
        let text = output.to_string();
        assert!(text.contains("file.txt:10-12 (of 20 lines)"));
        assert!(text.contains("  10  first line"));
        assert!(text.contains("  11  second line"));
        assert!(text.contains("  12  third line"));
    }

    #[test]
    fn test_render_json_range() {
        let report = ReadReport::Range {
            path: "data.txt".to_string(),
            start: 5,
            end: 6,
            total_lines: 10,
            lines: vec![LineEntry {
                line: 5,
                text: "test content".to_string(),
            }],
        };

        let output = report.render_json();
        let text = output.to_string();
        assert!(text.contains("\"start\""));
        assert!(text.contains("\"end\""));
        assert!(text.contains("\"lines\""));
    }

    #[test]
    fn test_render_plain_full() {
        let report = ReadReport::Full {
            path: "complete.txt".to_string(),
            total_lines: 2,
            lines: vec![
                LineEntry {
                    line: 1,
                    text: "first".to_string(),
                },
                LineEntry {
                    line: 2,
                    text: "second".to_string(),
                },
            ],
        };

        let output = report.render_plain();
        let text = output.to_string();
        assert!(text.contains("complete.txt (2 lines)"));
        assert!(text.contains("   1  first"));
        assert!(text.contains("   2  second"));
    }

    #[test]
    fn test_render_json_full() {
        let report = ReadReport::Full {
            path: "full.rs".to_string(),
            total_lines: 1,
            lines: vec![LineEntry {
                line: 1,
                text: "fn main() {}".to_string(),
            }],
        };

        let output = report.render_json();
        let text = output.to_string();
        assert!(text.contains("\"path\""));
        assert!(text.contains("\"lines\""));
        assert!(text.contains("fn main() {}"));
    }

    #[test]
    fn test_render_human_symbols() {
        let report = ReadReport::Symbols {
            path: "lib.rs".to_string(),
            total_lines: 0,
            symbols: vec![],
        };

        let plain = report.render_plain();
        let human = report.render_human();
        assert_eq!(plain.to_string(), human.to_string());
    }
}
