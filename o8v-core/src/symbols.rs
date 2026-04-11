// Copyright (c) 2026 Soheil Alizadeh / 8Network. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1).
// See LICENSE file in the project root.

//! Symbol extraction — parses source code to extract function/struct/class/method
//! signatures across multiple languages.
//!
//! Supports: Rust, Python, Go, TypeScript, JavaScript, C#, Ruby, Java, Kotlin, Swift.

// ─── Symbol ─────────────────────────────────────────────────────────────────

/// A named symbol found in source code.
pub struct Symbol {
    pub name: String,
    pub kind: String,
    pub line: usize,
    pub signature: String,
}

// ─── Symbol Extraction ───────────────────────────────────────────────────────

/// Extract symbols from source code by file extension.
///
/// Returns all function, struct, class, and method signatures found in
/// `content`. The `extension` parameter selects the language-specific
/// pattern set (e.g. `"rs"`, `"py"`, `"go"`).
pub fn extract_symbols(content: &str, extension: &str) -> Vec<Symbol> {
    let patterns: &[(&str, &str)] = match extension {
        "rs" => &[
            ("fn ", "function"),
            ("struct ", "struct"),
            ("enum ", "enum"),
            ("trait ", "trait"),
            ("impl ", "impl"),
            ("mod ", "mod"),
            ("type ", "type"),
        ],
        "py" => &[("def ", "function"), ("class ", "class")],
        "go" => &[("func ", "function"), ("type ", "type")],
        "ts" | "tsx" | "js" | "jsx" => &[
            ("export default function ", "function"),
            ("export default class ", "class"),
            ("export function ", "function"),
            ("export class ", "class"),
            ("export interface ", "interface"),
            ("function ", "function"),
            ("class ", "class"),
            ("interface ", "interface"),
        ],
        "cs" => &[
            ("class ", "class"),
            ("interface ", "interface"),
            ("struct ", "struct"),
            ("enum ", "enum"),
        ],
        "rb" => &[
            ("def ", "function"),
            ("class ", "class"),
            ("module ", "module"),
        ],
        "java" | "kt" => &[
            ("class ", "class"),
            ("interface ", "interface"),
            ("enum ", "enum"),
        ],
        "swift" => &[
            ("func ", "function"),
            ("class ", "class"),
            ("struct ", "struct"),
            ("enum ", "enum"),
            ("protocol ", "protocol"),
        ],
        _ => &[],
    };

    let mut symbols = Vec::new();
    let mut in_block_comment = false;

    for (line_num, line) in content.lines().enumerate() {
        // Strip \r so that Windows CRLF line endings do not confuse the
        // block-comment state machine. str::lines() splits on \r\n as a unit
        // but bare \r characters (old Mac line endings) would survive as
        // trailing bytes; trim() removes all ASCII whitespace including \r.
        let trimmed = line.trim();

        // Track block comment state
        if in_block_comment {
            if trimmed.contains("*/") {
                in_block_comment = false;
            }
            continue;
        }

        // Check for block comment start
        if trimmed.starts_with("/*") {
            if !trimmed.contains("*/") {
                in_block_comment = true;
            }
            continue;
        }

        // Skip single-line comments
        if trimmed.starts_with("//") || trimmed.starts_with('#') || trimmed.starts_with('*') {
            continue;
        }

        // Go receiver methods: "func (c *Config) Run()"
        if extension == "go" && trimmed.starts_with("func (") {
            if let Some(close_paren) = trimmed[5..].find(") ") {
                let after_receiver = &trimmed[5 + close_paren + 2..];
                let name: String = after_receiver
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                if !name.is_empty() {
                    symbols.push(Symbol {
                        name,
                        kind: "method".to_string(),
                        line: line_num + 1,
                        signature: trimmed.to_string(),
                    });
                }
            }
            continue;
        }

        for &(pattern, kind) in patterns {
            let prefixes = [
                "".to_string(),
                "pub ".to_string(),
                "pub(crate) ".to_string(),
                "async ".to_string(),
                "pub async ".to_string(),
                "pub(crate) async ".to_string(),
            ];
            let matched = prefixes.iter().find_map(|prefix| {
                let candidate = format!("{prefix}{pattern}");
                if trimmed.starts_with(candidate.as_str()) {
                    Some(candidate)
                } else {
                    None
                }
            });
            if let Some(candidate) = matched {
                let after_keyword = &trimmed[candidate.len()..];
                let name: String = after_keyword
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                if !name.is_empty() {
                    symbols.push(Symbol {
                        name,
                        kind: kind.to_string(),
                        line: line_num + 1,
                        signature: trimmed.to_string(),
                    });
                }
                break;
            }
        }
    }
    symbols
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rust_functions_and_structs() {
        let src = r#"
pub fn hello() {}
struct Foo {}
pub(crate) async fn bar() {}
// fn skipped_comment() {}
"#;
        let syms = extract_symbols(src, "rs");
        let names: Vec<&str> = syms.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"hello"), "expected hello, got {names:?}");
        assert!(names.contains(&"Foo"), "expected Foo, got {names:?}");
        assert!(names.contains(&"bar"), "expected bar, got {names:?}");
        assert!(
            !names.contains(&"skipped_comment"),
            "comment line must be skipped"
        );
    }

    #[test]
    fn go_receiver_methods() {
        let src = "func (c *Config) Run() error {}\nfunc plain() {}\n";
        let syms = extract_symbols(src, "go");
        let kinds: Vec<(&str, &str)> = syms
            .iter()
            .map(|s| (s.name.as_str(), s.kind.as_str()))
            .collect();
        assert!(
            kinds.contains(&("Run", "method")),
            "expected receiver method Run, got {kinds:?}"
        );
        assert!(
            kinds.contains(&("plain", "function")),
            "expected function plain, got {kinds:?}"
        );
    }

    #[test]
    fn block_comment_skipped() {
        let src = "/* fn inside_comment() {} */\nfn real() {}\n";
        let syms = extract_symbols(src, "rs");
        let names: Vec<&str> = syms.iter().map(|s| s.name.as_str()).collect();
        assert!(
            !names.contains(&"inside_comment"),
            "block comment must be skipped"
        );
        assert!(names.contains(&"real"), "expected real, got {names:?}");
    }

    #[test]
    fn unknown_extension_returns_empty() {
        let src = "fn something() {}\n";
        let syms = extract_symbols(src, "xyz");
        assert!(syms.is_empty(), "unknown extension must yield no symbols");
    }

    #[test]
    fn python_defs_and_classes() {
        let src = "def foo():\n    pass\nclass Bar:\n    pass\n";
        let syms = extract_symbols(src, "py");
        let names: Vec<&str> = syms.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"foo"));
        assert!(names.contains(&"Bar"));
    }
}
