//! Tests for ALL parsers — each uses real or realistic tool output fixtures.

use o8v_core::diagnostic::{Location, ParseStatus, Severity};

// ─── Cargo / Clippy parser ──────────────────────────────────────────────

fn parse_cargo(stdout: &str, root: &str) -> o8v_core::ParseResult {
    o8v_stacks::parse::cargo::parse(stdout, "", std::path::Path::new(root), "clippy", "rust")
}

#[test]
fn cargo_parse_real_clippy_output() {
    let stdout = include_str!("fixtures/parse/clippy.json");
    let result = parse_cargo(stdout, "/tmp/cargo-json-test");

    assert_eq!(
        result.status,
        ParseStatus::Parsed,
        "clippy output should parse cleanly"
    );
    assert!(!result.diagnostics.is_empty(), "should have diagnostics");

    let d = &result.diagnostics[0];
    assert!(
        matches!(&d.location, Location::File(f) if f == "src/main.rs"),
        "location should be src/main.rs"
    );
    assert_eq!(d.severity, Severity::Error, "severity should be Error");
    assert!(d.rule.is_some(), "rule should be present");
    assert!(
        d.message.contains("format!"),
        "message should mention format!"
    );
    assert!(!d.suggestions.is_empty(), "should have suggestions");
    assert_eq!(d.tool, "clippy", "tool should be clippy");
    assert_eq!(d.stack, "rust", "stack should be rust");
}

#[test]
fn cargo_parse_empty_output() {
    let result = parse_cargo("", "/tmp");
    assert_eq!(
        result.status,
        ParseStatus::Parsed,
        "empty input should parse cleanly"
    );
    assert!(
        result.diagnostics.is_empty(),
        "empty input should produce no diagnostics"
    );
}

#[test]
fn cargo_parse_build_finished_only() {
    let stdout = r#"{"reason":"build-finished","success":true}"#;
    let result = parse_cargo(stdout, "/tmp");
    assert_eq!(
        result.status,
        ParseStatus::Parsed,
        "build-finished message should parse cleanly"
    );
    assert!(
        result.diagnostics.is_empty(),
        "build-finished should produce no diagnostics"
    );
}

#[test]
fn cargo_parse_garbage_input() {
    let result = parse_cargo("not json at all\nmore garbage", "/tmp");
    assert_eq!(
        result.status,
        ParseStatus::Unparsed,
        "garbage input should be Unparsed"
    );
    assert!(
        result.diagnostics.is_empty(),
        "garbage input should produce no diagnostics"
    );
}

#[test]
fn cargo_parse_mixed_valid_invalid() {
    let stdout = format!(
        "{}\nnot json\n",
        include_str!("fixtures/parse/clippy.json")
            .lines()
            .next()
            .unwrap_or("")
    );
    let result = parse_cargo(&stdout, "/tmp/cargo-json-test");
    assert_eq!(
        result.status,
        ParseStatus::Parsed,
        "mixed input with valid JSON lines should parse"
    );
    assert!(
        !result.diagnostics.is_empty(),
        "should have diagnostics from valid lines"
    );
}

#[test]
fn cargo_path_normalization_relative() {
    let stdout = r#"{"reason":"compiler-message","package_id":"test","manifest_path":"","target":{"kind":["bin"],"crate_types":["bin"],"name":"t","src_path":"","edition":"2021","doc":true,"doctest":false,"test":true},"message":{"rendered":"","$message_type":"diagnostic","children":[],"level":"error","message":"test error","spans":[{"byte_end":10,"byte_start":0,"column_end":10,"column_start":1,"expansion":null,"file_name":"src/main.rs","is_primary":true,"label":null,"line_end":1,"line_start":1,"suggested_replacement":null,"suggestion_applicability":null,"text":[]}],"code":{"code":"E0001","explanation":null}}}"#;
    let result = parse_cargo(stdout, "/project");
    assert_eq!(
        result.diagnostics.len(),
        1,
        "should parse exactly one diagnostic"
    );
    assert!(
        matches!(&result.diagnostics[0].location, Location::File(f) if f == "src/main.rs"),
        "relative path should be preserved as-is"
    );
}

#[test]
fn cargo_suggestions_have_edits() {
    let stdout = include_str!("fixtures/parse/clippy.json");
    let result = parse_cargo(stdout, "/tmp/cargo-json-test");

    for d in &result.diagnostics {
        for s in &d.suggestions {
            if !s.edits.is_empty() {
                for e in &s.edits {
                    assert!(e.span.line > 0, "edit span line should be 1-indexed");
                }
            }
        }
    }
}

#[test]
fn cargo_notes_extracted_from_children() {
    let stdout = include_str!("fixtures/parse/clippy.json");
    let result = parse_cargo(stdout, "/tmp/cargo-json-test");

    let d = &result.diagnostics[0];
    assert!(
        !d.notes.is_empty(),
        "should have notes from children: {d:?}"
    );
}

// ─── Ruff parser (real output from ruff check --output-format=json) ──────

#[test]
fn ruff_parses_real_output() {
    let stdout = include_str!("fixtures/parse/ruff.json");
    let result = o8v_stacks::parse::ruff::parse(
        stdout,
        "",
        std::path::Path::new("/tmp/8v-fixtures/ruff"),
        "ruff",
        "python",
    );

    assert_eq!(
        result.status,
        ParseStatus::Parsed,
        "ruff output should parse cleanly"
    );
    assert!(!result.diagnostics.is_empty(), "should have diagnostics");

    let d = &result.diagnostics[0];
    assert!(
        matches!(&d.location, Location::File(f) if f.contains("bad.py")),
        "location should point to bad.py"
    );
    assert!(d.span.is_some(), "span should be present");
    assert!(d.rule.is_some(), "rule should be present");
    assert_eq!(d.severity, Severity::Error, "severity should be Error");
    assert!(!d.message.is_empty(), "message should not be empty");
    assert_eq!(d.tool, "ruff", "tool should be ruff");
    assert_eq!(d.stack, "python", "stack should be python");
}

#[test]
fn ruff_has_fix_suggestions() {
    let stdout = include_str!("fixtures/parse/ruff.json");
    let result =
        o8v_stacks::parse::ruff::parse(stdout, "", std::path::Path::new("/tmp"), "ruff", "python");

    let has_suggestions = result.diagnostics.iter().any(|d| !d.suggestions.is_empty());
    assert!(
        has_suggestions,
        "ruff output should contain fixable violations with suggestions"
    );
}

#[test]
fn ruff_empty_array() {
    let result =
        o8v_stacks::parse::ruff::parse("[]", "", std::path::Path::new("/tmp"), "ruff", "python");

    assert_eq!(
        result.status,
        ParseStatus::Parsed,
        "empty array should parse cleanly"
    );
    assert!(
        result.diagnostics.is_empty(),
        "empty array should produce no diagnostics"
    );
}

#[test]
fn ruff_invalid_json() {
    let result = o8v_stacks::parse::ruff::parse(
        "not json",
        "",
        std::path::Path::new("/tmp"),
        "ruff",
        "python",
    );

    assert_eq!(
        result.status,
        ParseStatus::Unparsed,
        "invalid JSON should be Unparsed"
    );
    assert!(
        result.diagnostics.is_empty(),
        "invalid JSON should produce no diagnostics"
    );
}

// ─── Go vet parser (real output from go vet -json) ───────────────────────

#[test]
fn govet_parses_real_output() {
    let stdout = include_str!("fixtures/parse/govet.json");
    let result = o8v_stacks::parse::govet::parse(
        stdout,
        "",
        std::path::Path::new("/tmp/8v-fixtures/govet"),
        "go vet",
        "go",
    );

    assert_eq!(
        result.status,
        ParseStatus::Parsed,
        "go vet output should parse cleanly"
    );
    assert!(
        !result.diagnostics.is_empty(),
        "should have diagnostics from go vet"
    );

    let d = &result.diagnostics[0];
    assert!(d.rule.is_some(), "rule should be the analyzer name");
    assert!(!d.message.is_empty(), "message should not be empty");
    assert_eq!(d.severity, Severity::Error, "severity should be Error");
    assert_eq!(d.tool, "go vet", "tool should be go vet");
    assert_eq!(d.stack, "go", "stack should be go");
}

#[test]
fn govet_empty_input() {
    let result =
        o8v_stacks::parse::govet::parse("", "", std::path::Path::new("/tmp"), "go vet", "go");

    assert_eq!(
        result.status,
        ParseStatus::Parsed,
        "empty input should parse cleanly"
    );
    assert!(
        result.diagnostics.is_empty(),
        "empty input should produce no diagnostics"
    );
}

#[test]
fn govet_invalid_json() {
    let result = o8v_stacks::parse::govet::parse(
        "not json",
        "",
        std::path::Path::new("/tmp"),
        "go vet",
        "go",
    );

    assert_eq!(
        result.status,
        ParseStatus::Unparsed,
        "invalid JSON should be Unparsed"
    );
    assert!(
        result.diagnostics.is_empty(),
        "invalid JSON should produce no diagnostics"
    );
}

// ─── TSC parser (realistic output matching tsc --noEmit --pretty false) ──

#[test]
fn tsc_parses_diagnostic_lines() {
    let stdout = include_str!("fixtures/parse/tsc.txt");
    let result = o8v_stacks::parse::tsc::parse(
        stdout,
        "",
        std::path::Path::new("/project"),
        "tsc",
        "typescript",
    );

    assert_eq!(
        result.status,
        ParseStatus::Parsed,
        "tsc output should parse cleanly"
    );
    assert_eq!(
        result.diagnostics.len(),
        2,
        "should parse exactly two diagnostics"
    );

    assert!(
        matches!(&result.diagnostics[0].location, Location::File(f) if f == "src/index.ts"),
        "first diagnostic should point to src/index.ts"
    );
    assert_eq!(
        result.diagnostics[0].span.as_ref().unwrap().line,
        3,
        "first diagnostic line should be 3"
    );
    assert_eq!(
        result.diagnostics[0].span.as_ref().unwrap().column,
        1,
        "first diagnostic column should be 1"
    );
    assert_eq!(
        result.diagnostics[0].rule.as_deref(),
        Some("TS2304"),
        "first diagnostic rule should be TS2304"
    );
    assert_eq!(
        result.diagnostics[0].severity,
        Severity::Error,
        "first diagnostic severity should be Error"
    );
    assert!(
        result.diagnostics[0].message.contains("foo"),
        "first diagnostic message should contain foo"
    );

    assert_eq!(
        result.diagnostics[1].span.as_ref().unwrap().line,
        7,
        "second diagnostic line should be 7"
    );
    assert_eq!(
        result.diagnostics[1].rule.as_deref(),
        Some("TS2345"),
        "second diagnostic rule should be TS2345"
    );
}

#[test]
fn tsc_filename_with_parens() {
    let line = "handler(1).ts(3,5): error TS2304: Cannot find name 'foo'";
    let result = o8v_stacks::parse::tsc::parse(
        line,
        "",
        std::path::Path::new("/project"),
        "tsc",
        "typescript",
    );
    assert_eq!(
        result.status,
        ParseStatus::Parsed,
        "tsc line should parse cleanly"
    );
    assert_eq!(
        result.diagnostics.len(),
        1,
        "should parse exactly one diagnostic"
    );
    assert!(
        matches!(&result.diagnostics[0].location, Location::File(f) if f == "handler(1).ts"),
        "filename should include parens: {:?}",
        result.diagnostics[0].location
    );
    assert_eq!(
        result.diagnostics[0].span.as_ref().unwrap().line,
        3,
        "diagnostic line should be 3"
    );
    assert_eq!(
        result.diagnostics[0].span.as_ref().unwrap().column,
        5,
        "diagnostic column should be 5"
    );
}

#[test]
fn tsc_empty_output() {
    let result =
        o8v_stacks::parse::tsc::parse("", "", std::path::Path::new("/tmp"), "tsc", "typescript");

    assert_eq!(
        result.status,
        ParseStatus::Parsed,
        "empty output should parse cleanly"
    );
    assert!(
        result.diagnostics.is_empty(),
        "empty output should produce no diagnostics"
    );
}

#[test]
fn tsc_non_diagnostic_text() {
    let result = o8v_stacks::parse::tsc::parse(
        "Some random compiler noise\nAnother line",
        "",
        std::path::Path::new("/tmp"),
        "tsc",
        "typescript",
    );

    assert_eq!(
        result.status,
        ParseStatus::Parsed,
        "non-diagnostic text should parse cleanly"
    );
    assert!(
        result.diagnostics.is_empty(),
        "non-diagnostic text should produce no diagnostics"
    );
}

#[test]
fn tsc_parses_stderr_when_stdout_empty() {
    let stderr =
        "src/main.ts(3,5): error TS2322: Type 'string' is not assignable to type 'number'.";
    let result = o8v_stacks::parse::tsc::parse(
        "",
        stderr,
        std::path::Path::new("/project"),
        "deno check",
        "deno",
    );

    assert_eq!(
        result.status,
        ParseStatus::Parsed,
        "stderr tsc output should parse cleanly"
    );
    assert_eq!(result.diagnostics.len(), 1, "should parse from stderr");
    assert_eq!(
        result.diagnostics[0].span.as_ref().unwrap().line,
        3,
        "diagnostic line should be 3"
    );
    assert_eq!(
        result.diagnostics[0].rule.as_deref(),
        Some("TS2322"),
        "rule should be TS2322"
    );
    assert_eq!(
        result.diagnostics[0].tool, "deno check",
        "tool should be deno check"
    );
}

// ─── Dotnet parser (realistic MSBuild output) ────────────────────────────

#[test]
fn dotnet_parses_diagnostic_lines() {
    let stdout = include_str!("fixtures/parse/dotnet.txt");
    let result = o8v_stacks::parse::dotnet::parse(
        stdout,
        "",
        std::path::Path::new("/tmp"),
        "dotnet build",
        "dotnet",
    );

    assert_eq!(
        result.status,
        ParseStatus::Parsed,
        "dotnet output should parse cleanly"
    );
    assert_eq!(
        result.diagnostics.len(),
        2,
        "should parse exactly two diagnostics"
    );

    assert!(
        matches!(&result.diagnostics[0].location, Location::File(f) if f == "Program.cs"),
        "first diagnostic should point to Program.cs"
    );
    assert_eq!(
        result.diagnostics[0].span.as_ref().unwrap().line,
        1,
        "first diagnostic line should be 1"
    );
    assert_eq!(
        result.diagnostics[0].span.as_ref().unwrap().column,
        1,
        "first diagnostic column should be 1"
    );
    assert_eq!(
        result.diagnostics[0].rule.as_deref(),
        Some("CS8805"),
        "first diagnostic rule should be CS8805"
    );
    assert_eq!(
        result.diagnostics[0].severity,
        Severity::Error,
        "first diagnostic severity should be Error"
    );

    assert_eq!(
        result.diagnostics[1].rule.as_deref(),
        Some("CS0103"),
        "second diagnostic rule should be CS0103"
    );
}

#[test]
fn dotnet_build_noise_is_parsed_clean() {
    let stdout = "  Determining projects to restore...\n  Restored /tmp/App.csproj (in 106 ms).\n  Build succeeded.\n";
    let result = o8v_stacks::parse::dotnet::parse(
        stdout,
        "",
        std::path::Path::new("/tmp"),
        "dotnet build",
        "dotnet",
    );

    assert_eq!(
        result.status,
        ParseStatus::Parsed,
        "build noise should parse cleanly"
    );
    assert!(
        result.diagnostics.is_empty(),
        "build noise should produce no diagnostics"
    );
}

#[test]
fn dotnet_message_with_brackets_preserved() {
    let line = "file.cs(1,1): error CS0001: Expected [int] not [string]";
    let result = o8v_stacks::parse::dotnet::parse(
        line,
        "",
        std::path::Path::new("/tmp"),
        "dotnet build",
        "dotnet",
    );
    assert_eq!(
        result.status,
        ParseStatus::Parsed,
        "dotnet line with brackets should parse cleanly"
    );
    assert_eq!(
        result.diagnostics.len(),
        1,
        "should parse exactly one diagnostic"
    );
    assert!(
        result.diagnostics[0].message.contains("[int]"),
        "brackets in message must survive: {}",
        result.diagnostics[0].message
    );
    assert!(
        result.diagnostics[0].message.contains("[string]"),
        "brackets in message must survive: {}",
        result.diagnostics[0].message
    );
}

#[test]
fn dotnet_project_suffix_stripped() {
    let line = "file.cs(1,1): error CS0001: some error [/path/to/project.csproj]";
    let result = o8v_stacks::parse::dotnet::parse(
        line,
        "",
        std::path::Path::new("/tmp"),
        "dotnet build",
        "dotnet",
    );
    assert_eq!(
        result.status,
        ParseStatus::Parsed,
        "dotnet line should parse cleanly"
    );
    assert_eq!(
        result.diagnostics.len(),
        1,
        "should parse exactly one diagnostic"
    );
    assert!(
        !result.diagnostics[0].message.contains("project.csproj"),
        "project suffix should be stripped: {}",
        result.diagnostics[0].message
    );
}

#[test]
fn dotnet_empty_output() {
    let result = o8v_stacks::parse::dotnet::parse(
        "",
        "",
        std::path::Path::new("/tmp"),
        "dotnet build",
        "dotnet",
    );

    assert_eq!(
        result.status,
        ParseStatus::Parsed,
        "empty output should parse cleanly"
    );
    assert!(
        result.diagnostics.is_empty(),
        "empty output should produce no diagnostics"
    );
}

// ─── ESLint parser (synthetic but matching real schema) ──────────────────

#[test]
fn eslint_parses_json_array() {
    let stdout = r#"[{
        "filePath": "/project/src/index.js",
        "messages": [
            {
                "ruleId": "no-unused-vars",
                "severity": 2,
                "message": "'x' is defined but never used",
                "line": 1,
                "column": 5,
                "endLine": 1,
                "endColumn": 6
            },
            {
                "ruleId": "no-console",
                "severity": 1,
                "message": "Unexpected console statement",
                "line": 2,
                "column": 1,
                "endLine": 2,
                "endColumn": 20
            }
        ]
    }]"#;

    let result = o8v_stacks::parse::eslint::parse(
        stdout,
        "",
        std::path::Path::new("/project"),
        "eslint",
        "javascript",
    );

    assert_eq!(
        result.status,
        ParseStatus::Parsed,
        "eslint output should parse cleanly"
    );
    assert_eq!(
        result.diagnostics.len(),
        2,
        "should parse exactly two diagnostics"
    );

    assert!(
        matches!(&result.diagnostics[0].location, Location::File(f) if f == "src/index.js"),
        "first diagnostic should point to src/index.js"
    );
    assert_eq!(
        result.diagnostics[0].span.as_ref().unwrap().line,
        1,
        "first diagnostic line should be 1"
    );
    assert_eq!(
        result.diagnostics[0].rule.as_deref(),
        Some("no-unused-vars"),
        "first diagnostic rule should be no-unused-vars"
    );
    assert_eq!(
        result.diagnostics[0].severity,
        Severity::Error,
        "severity 2 should map to Error"
    );

    assert_eq!(
        result.diagnostics[1].rule.as_deref(),
        Some("no-console"),
        "second diagnostic rule should be no-console"
    );
    assert_eq!(
        result.diagnostics[1].severity,
        Severity::Warning,
        "severity 1 should map to Warning"
    );
}

#[test]
fn eslint_empty_array() {
    let result = o8v_stacks::parse::eslint::parse(
        "[]",
        "",
        std::path::Path::new("/tmp"),
        "eslint",
        "javascript",
    );

    assert_eq!(
        result.status,
        ParseStatus::Parsed,
        "empty array should parse cleanly"
    );
    assert!(
        result.diagnostics.is_empty(),
        "empty array should produce no diagnostics"
    );
}

#[test]
fn eslint_invalid_json() {
    let result = o8v_stacks::parse::eslint::parse(
        "not json",
        "",
        std::path::Path::new("/tmp"),
        "eslint",
        "javascript",
    );

    assert_eq!(
        result.status,
        ParseStatus::Unparsed,
        "invalid JSON should be Unparsed"
    );
    assert!(
        result.diagnostics.is_empty(),
        "invalid JSON should produce no diagnostics"
    );
}

// ─── Staticcheck parser (synthetic NDJSON matching real schema) ──────────

#[test]
fn staticcheck_parses_ndjson() {
    let stdout = r#"{"code":"SA4000","severity":"error","location":{"file":"/project/main.go","line":10,"column":5},"end":{"file":"/project/main.go","line":10,"column":15},"message":"identical expressions on the left and right side of the '==' operator"}
{"code":"S1000","severity":"warning","location":{"file":"/project/util.go","line":3,"column":1},"end":{"file":"/project/util.go","line":3,"column":10},"message":"should use for range instead of for { select {} }"}"#;

    let result = o8v_stacks::parse::staticcheck::parse(
        stdout,
        "",
        std::path::Path::new("/project"),
        "staticcheck",
        "go",
    );

    assert_eq!(
        result.status,
        ParseStatus::Parsed,
        "staticcheck output should parse cleanly"
    );
    assert_eq!(
        result.diagnostics.len(),
        2,
        "should parse exactly two diagnostics"
    );

    assert!(
        matches!(&result.diagnostics[0].location, Location::File(f) if f == "main.go"),
        "first diagnostic should point to main.go"
    );
    assert_eq!(
        result.diagnostics[0].rule.as_deref(),
        Some("SA4000"),
        "first diagnostic rule should be SA4000"
    );
    assert_eq!(
        result.diagnostics[0].severity,
        Severity::Error,
        "first diagnostic severity should be Error"
    );

    assert!(
        matches!(&result.diagnostics[1].location, Location::File(f) if f == "util.go"),
        "second diagnostic should point to util.go"
    );
    assert_eq!(
        result.diagnostics[1].rule.as_deref(),
        Some("S1000"),
        "second diagnostic rule should be S1000"
    );
    assert_eq!(
        result.diagnostics[1].severity,
        Severity::Warning,
        "second diagnostic severity should be Warning"
    );
}

#[test]
fn staticcheck_empty() {
    let result = o8v_stacks::parse::staticcheck::parse(
        "",
        "",
        std::path::Path::new("/tmp"),
        "staticcheck",
        "go",
    );

    assert_eq!(
        result.status,
        ParseStatus::Parsed,
        "empty input should parse cleanly"
    );
    assert!(
        result.diagnostics.is_empty(),
        "empty input should produce no diagnostics"
    );
}

// ─── Rustfmt parser (output from cargo fmt --all --check -- --color=never) ──

fn parse_rustfmt(stdout: &str, root: &str) -> o8v_core::ParseResult {
    o8v_stacks::parse::rustfmt::parse(stdout, "", std::path::Path::new(root), "cargo fmt", "rust")
}

#[test]
fn rustfmt_parses_single_diff_block() {
    let stdout = "\
Diff in /project/src/main.rs:1:
 fn main() {
-    let x=1;
+    let x = 1;
 }
";
    let result = parse_rustfmt(stdout, "/project");

    assert_eq!(
        result.status,
        ParseStatus::Parsed,
        "rustfmt diff should parse cleanly"
    );
    assert_eq!(
        result.diagnostics.len(),
        1,
        "should parse exactly one diagnostic"
    );

    let d = &result.diagnostics[0];
    assert!(
        matches!(&d.location, Location::File(f) if f == "src/main.rs"),
        "unexpected location: {:?}",
        d.location
    );
    assert_eq!(
        d.span.as_ref().unwrap().line,
        1,
        "diagnostic line should be 1"
    );
    assert_eq!(d.severity, Severity::Error, "severity should be Error");
    assert_eq!(d.tool, "cargo fmt", "tool should be cargo fmt");
    assert_eq!(d.stack, "rust", "stack should be rust");
    assert!(
        d.snippet.as_ref().unwrap().contains("-    let x=1;"),
        "snippet missing removal line"
    );
    assert!(
        d.snippet.as_ref().unwrap().contains("+    let x = 1;"),
        "snippet missing addition line"
    );
}

#[test]
fn rustfmt_parses_multiple_diff_blocks() {
    let stdout = "\
Diff in /project/src/a.rs:5:
-bad_format(  );
+bad_format();
Diff in /project/src/b.rs:12:
-fn foo(){
+fn foo() {
";
    let result = parse_rustfmt(stdout, "/project");

    assert_eq!(
        result.status,
        ParseStatus::Parsed,
        "multiple rustfmt diffs should parse cleanly"
    );
    assert_eq!(
        result.diagnostics.len(),
        2,
        "should parse exactly two diagnostics"
    );

    assert!(
        matches!(&result.diagnostics[0].location, Location::File(f) if f == "src/a.rs"),
        "unexpected location: {:?}",
        result.diagnostics[0].location
    );
    assert_eq!(
        result.diagnostics[0].span.as_ref().unwrap().line,
        5,
        "first diagnostic line should be 5"
    );

    assert!(
        matches!(&result.diagnostics[1].location, Location::File(f) if f == "src/b.rs"),
        "unexpected location: {:?}",
        result.diagnostics[1].location
    );
    assert_eq!(
        result.diagnostics[1].span.as_ref().unwrap().line,
        12,
        "second diagnostic line should be 12"
    );
}

#[test]
fn rustfmt_empty_output_returns_parsed_empty() {
    let result = parse_rustfmt("", "/project");

    assert_eq!(
        result.status,
        ParseStatus::Parsed,
        "empty output should parse cleanly"
    );
    assert!(
        result.diagnostics.is_empty(),
        "empty output should produce no diagnostics"
    );
}

#[test]
fn rustfmt_non_diff_text_returns_parsed_clean() {
    let stdout = "Checking formatting...\nAll files are formatted correctly.\n";
    let result = parse_rustfmt(stdout, "/project");

    // Text parser scanned every line for "Diff in" headers — found none → clean.
    assert_eq!(
        result.status,
        ParseStatus::Parsed,
        "non-diff text should parse cleanly"
    );
    assert!(
        result.diagnostics.is_empty(),
        "non-diff text should produce no diagnostics"
    );
}
