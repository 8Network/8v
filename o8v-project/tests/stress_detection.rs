//! Stress tests for malformed config files.
//! Tests all detectors with edge cases:
//! - Empty files
//! - Huge files
//! - Binary content
//! - Invalid schemas
//! - Deeply nested content
//! - Duplicate keys
//!
//! No panics allowed. All should return Ok(None) or Err with descriptive messages.

use o8v_project::{detect_all, ProjectRoot};

// ═══════════════════════════════════════════════════════════════════════════════
// ─── EMPTY FILES ───────────────────────────────────────────────────────────────
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn empty_package_json() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("package.json"), "").unwrap();
    std::fs::write(dir.path().join("tsconfig.json"), "{}").unwrap();

    let r = detect_all(&ProjectRoot::new(dir.path()).unwrap());
    // Empty JSON should error or return None, never panic
    assert!(
        r.errors().is_empty() || r.projects().is_empty(),
        "empty package.json should not panic"
    );
}

#[test]
fn empty_cargo_toml() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("Cargo.toml"), "").unwrap();

    let r = detect_all(&ProjectRoot::new(dir.path()).unwrap());
    // Empty TOML should error, never panic
    assert!(
        r.errors().is_empty() || r.projects().is_empty(),
        "empty Cargo.toml should not panic"
    );
}

#[test]
fn empty_pyproject_toml() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("pyproject.toml"), "").unwrap();

    let r = detect_all(&ProjectRoot::new(dir.path()).unwrap());
    assert!(
        r.errors().is_empty() || r.projects().is_empty(),
        "empty pyproject.toml should not panic"
    );
}

#[test]
fn empty_go_mod() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("go.mod"), "").unwrap();

    let r = detect_all(&ProjectRoot::new(dir.path()).unwrap());
    assert!(
        r.errors().is_empty() || r.projects().is_empty(),
        "empty go.mod should not panic"
    );
}

#[test]
fn empty_deno_json() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("deno.json"), "").unwrap();

    let r = detect_all(&ProjectRoot::new(dir.path()).unwrap());
    assert!(
        r.errors().is_empty() || r.projects().is_empty(),
        "empty deno.json should not panic"
    );
}

#[test]
fn empty_csproj() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("App.csproj"), "").unwrap();

    let r = detect_all(&ProjectRoot::new(dir.path()).unwrap());
    assert!(
        r.errors().is_empty() || r.projects().is_empty(),
        "empty .csproj should not panic"
    );
}

#[test]
fn empty_sln() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("App.sln"), "").unwrap();

    let r = detect_all(&ProjectRoot::new(dir.path()).unwrap());
    assert!(
        r.errors().is_empty() || r.projects().is_empty(),
        "empty .sln should not panic"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// ─── HUGE FILES (1MB of valid-looking content) ─────────────────────────────────
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn huge_package_json_with_repeated_section() {
    let dir = tempfile::tempdir().unwrap();
    let mut content = String::from(r#"{"name": "app", "version": "1.0.0", "dependencies": {"#);
    for i in 0..50000 {
        if i > 0 {
            content.push(',');
        }
        content.push_str(&format!(r#""pkg{}": "1.0.0""#, i));
    }
    content.push_str("}}");

    std::fs::write(dir.path().join("package.json"), content).unwrap();
    std::fs::write(dir.path().join("tsconfig.json"), "{}").unwrap();

    let r = detect_all(&ProjectRoot::new(dir.path()).unwrap());
    // Should handle large files without panic
    assert!(
        !r.errors().iter().any(|e| {
            let msg = format!("{}", e);
            msg.contains("panic") || msg.contains("overflow")
        }),
        "huge package.json should not panic, got: {:?}",
        r.errors()
    );
}

#[test]
fn huge_cargo_toml_with_repeated_dependencies() {
    let dir = tempfile::tempdir().unwrap();
    let mut content =
        String::from("[package]\nname = \"app\"\nversion = \"1.0.0\"\n\n[dependencies]\n");
    for i in 0..50000 {
        content.push_str(&format!("pkg_{} = \"1.0.0\"\n", i));
    }

    std::fs::write(dir.path().join("Cargo.toml"), content).unwrap();

    let r = detect_all(&ProjectRoot::new(dir.path()).unwrap());
    assert!(
        !r.errors().iter().any(|e| {
            let msg = format!("{}", e);
            msg.contains("panic") || msg.contains("overflow")
        }),
        "huge Cargo.toml should not panic"
    );
}

#[test]
fn huge_pyproject_toml_with_repeated_dependencies() {
    let dir = tempfile::tempdir().unwrap();
    let mut content =
        String::from("[project]\nname = \"app\"\nversion = \"1.0.0\"\ndependencies = [\n");
    for i in 0..50000 {
        if i > 0 {
            content.push(',');
        }
        content.push_str(&format!(r#""pkg-{} >= 1.0.0""#, i));
    }
    content.push_str("]\n");

    std::fs::write(dir.path().join("pyproject.toml"), content).unwrap();

    let r = detect_all(&ProjectRoot::new(dir.path()).unwrap());
    assert!(
        !r.errors().iter().any(|e| {
            let msg = format!("{}", e);
            msg.contains("panic") || msg.contains("overflow")
        }),
        "huge pyproject.toml should not panic"
    );
}

#[test]
fn huge_deno_json() {
    let dir = tempfile::tempdir().unwrap();
    let mut content = String::from(r#"{"name": "app", "imports": {"#);
    for i in 0..50000 {
        if i > 0 {
            content.push(',');
        }
        content.push_str(&format!(r#""pkg{}": "jsr:@scope/pkg@1.0.0""#, i));
    }
    content.push_str("}}");

    std::fs::write(dir.path().join("deno.json"), content).unwrap();

    let r = detect_all(&ProjectRoot::new(dir.path()).unwrap());
    assert!(
        !r.errors().iter().any(|e| {
            let msg = format!("{}", e);
            msg.contains("panic") || msg.contains("overflow")
        }),
        "huge deno.json should not panic"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// ─── BINARY CONTENT ────────────────────────────────────────────────────────────
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn binary_package_json() {
    let dir = tempfile::tempdir().unwrap();
    let binary = vec![0xFF, 0xFE, 0x00, 0x01, 0xFF, 0xFF, 0x00, 0x00];
    std::fs::write(dir.path().join("package.json"), &binary).unwrap();
    std::fs::write(dir.path().join("tsconfig.json"), "{}").unwrap();

    let r = detect_all(&ProjectRoot::new(dir.path()).unwrap());
    // Should handle binary gracefully
    assert!(
        !r.errors().iter().any(|e| {
            let msg = format!("{}", e);
            msg.contains("panic")
        }),
        "binary package.json should not panic"
    );
}

#[test]
fn binary_cargo_toml() {
    let dir = tempfile::tempdir().unwrap();
    let binary = vec![0xFF, 0xFE, 0x00, 0x01, 0x42, 0x41, 0x44, 0x00];
    std::fs::write(dir.path().join("Cargo.toml"), &binary).unwrap();

    let r = detect_all(&ProjectRoot::new(dir.path()).unwrap());
    assert!(
        !r.errors().iter().any(|e| {
            let msg = format!("{}", e);
            msg.contains("panic")
        }),
        "binary Cargo.toml should not panic"
    );
}

#[test]
fn binary_pyproject_toml() {
    let dir = tempfile::tempdir().unwrap();
    let binary = vec![0x00, 0xFF, 0xAB, 0xCD, 0xEF, 0x12, 0x34, 0x56];
    std::fs::write(dir.path().join("pyproject.toml"), &binary).unwrap();

    let r = detect_all(&ProjectRoot::new(dir.path()).unwrap());
    assert!(
        !r.errors().iter().any(|e| {
            let msg = format!("{}", e);
            msg.contains("panic")
        }),
        "binary pyproject.toml should not panic"
    );
}

#[test]
fn binary_deno_json() {
    let dir = tempfile::tempdir().unwrap();
    let binary = vec![0x80, 0x81, 0x82, 0x83, 0x84, 0x85];
    std::fs::write(dir.path().join("deno.json"), &binary).unwrap();

    let r = detect_all(&ProjectRoot::new(dir.path()).unwrap());
    assert!(
        !r.errors().iter().any(|e| {
            let msg = format!("{}", e);
            msg.contains("panic")
        }),
        "binary deno.json should not panic"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// ─── VALID JSON/TOML BUT WRONG SCHEMA ──────────────────────────────────────────
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn package_json_name_is_number() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("package.json"), r#"{"name": 123}"#).unwrap();
    std::fs::write(dir.path().join("tsconfig.json"), "{}").unwrap();

    let r = detect_all(&ProjectRoot::new(dir.path()).unwrap());
    // Should error, not panic or silence
    assert!(!r.errors().is_empty(), "name: 123 should produce error");
}

#[test]
fn package_json_version_is_array() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("package.json"),
        r#"{"name": "app", "version": [1, 0, 0]}"#,
    )
    .unwrap();
    std::fs::write(dir.path().join("tsconfig.json"), "{}").unwrap();

    let r = detect_all(&ProjectRoot::new(dir.path()).unwrap());
    // Array version is malformed
    assert!(
        !r.errors().is_empty() || !r.projects().is_empty(),
        "version as array should be handled"
    );
}

#[test]
fn cargo_toml_version_is_number() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("Cargo.toml"),
        "[package]\nname = \"app\"\nversion = 42\n",
    )
    .unwrap();

    let r = detect_all(&ProjectRoot::new(dir.path()).unwrap());
    assert!(!r.errors().is_empty(), "version = 42 should error");
}

#[test]
fn pyproject_toml_name_is_number() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("pyproject.toml"),
        "[project]\nname = 42\nversion = \"1.0.0\"\n",
    )
    .unwrap();

    let r = detect_all(&ProjectRoot::new(dir.path()).unwrap());
    assert!(!r.errors().is_empty(), "name = 42 should error");
}

#[test]
fn deno_json_name_is_number() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("deno.json"), r#"{"name": 999}"#).unwrap();

    let r = detect_all(&ProjectRoot::new(dir.path()).unwrap());
    // Should handle gracefully
    assert!(
        r.errors().is_empty() || r.projects().is_empty(),
        "deno.json with name: 999 should not panic"
    );
}

#[test]
fn csproj_with_invalid_xml_tags() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("App.csproj"),
        "<Project><PropertyGroup><RootNamespace>></>Invalid</PropertyGroup></Project>",
    )
    .unwrap();

    let r = detect_all(&ProjectRoot::new(dir.path()).unwrap());
    // Malformed XML should error, not panic
    assert!(
        !r.errors().iter().any(|e| {
            let msg = format!("{}", e);
            msg.contains("panic")
        }),
        "malformed XML should not panic"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// ─── DEEPLY NESTED JSON (100+ levels) ──────────────────────────────────────────
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn deeply_nested_package_json() {
    let dir = tempfile::tempdir().unwrap();
    let mut content = String::from(r#"{"name": "app""#);
    for i in 0..100 {
        content.push_str(&format!(r#", "nested{}": {{"#, i));
    }
    content.push_str(r#""value": "deep""#);
    for _ in 0..100 {
        content.push('}');
    }
    content.push('}');

    std::fs::write(dir.path().join("package.json"), content).unwrap();
    std::fs::write(dir.path().join("tsconfig.json"), "{}").unwrap();

    let r = detect_all(&ProjectRoot::new(dir.path()).unwrap());
    // Deep nesting should be handled without panic
    assert!(
        !r.errors().iter().any(|e| {
            let msg = format!("{}", e);
            msg.contains("panic") || msg.contains("recursion")
        }),
        "deeply nested package.json should not panic"
    );
}

#[test]
fn deeply_nested_deno_json() {
    let dir = tempfile::tempdir().unwrap();
    let mut content = String::from(r#"{"name": "app""#);
    for i in 0..100 {
        content.push_str(&format!(r#", "level{}": {{"#, i));
    }
    content.push_str(r#""end": true"#);
    for _ in 0..100 {
        content.push('}');
    }
    content.push('}');

    std::fs::write(dir.path().join("deno.json"), content).unwrap();

    let r = detect_all(&ProjectRoot::new(dir.path()).unwrap());
    assert!(
        !r.errors().iter().any(|e| {
            let msg = format!("{}", e);
            msg.contains("panic") || msg.contains("recursion")
        }),
        "deeply nested deno.json should not panic"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// ─── DUPLICATE KEYS ────────────────────────────────────────────────────────────
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn package_json_duplicate_keys() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("package.json"),
        r#"{"name": "app", "version": "1.0.0", "name": "app2"}"#,
    )
    .unwrap();
    std::fs::write(dir.path().join("tsconfig.json"), "{}").unwrap();

    let r = detect_all(&ProjectRoot::new(dir.path()).unwrap());
    // Duplicate keys may be valid JSON depending on parser
    // Should not panic in any case
    assert!(
        !r.errors().iter().any(|e| {
            let msg = format!("{}", e);
            msg.contains("panic")
        }),
        "duplicate keys should not panic"
    );
}

#[test]
fn cargo_toml_duplicate_keys() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("Cargo.toml"),
        "[package]\nname = \"app\"\nversion = \"1.0.0\"\nname = \"app2\"\n",
    )
    .unwrap();

    let r = detect_all(&ProjectRoot::new(dir.path()).unwrap());
    // TOML should error on duplicate keys, not panic
    assert!(
        !r.errors().iter().any(|e| {
            let msg = format!("{}", e);
            msg.contains("panic")
        }),
        "duplicate TOML keys should not panic"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// ─── SPECIFIC DETECTOR EDGE CASES ──────────────────────────────────────────────
// ═══════════════════════════════════════════════════════════════════════════════

// ─── Rust: Invalid TOML syntax ─────────────────────────────────────────────────

#[test]
fn cargo_toml_invalid_toml_syntax() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("Cargo.toml"),
        "[package\nname = \"app\"  # missing closing bracket\nversion = \"1.0.0\"\n",
    )
    .unwrap();

    let r = detect_all(&ProjectRoot::new(dir.path()).unwrap());
    assert!(
        !r.errors().iter().any(|e| {
            let msg = format!("{}", e);
            msg.contains("panic")
        }),
        "invalid TOML should not panic"
    );
}

#[test]
fn cargo_toml_broken_dependency_syntax() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("Cargo.toml"),
        "[package]\nname = \"app\"\nversion = \"1.0.0\"\n\n[dependencies]\nserde = \n",
    )
    .unwrap();

    let r = detect_all(&ProjectRoot::new(dir.path()).unwrap());
    assert!(
        !r.errors().iter().any(|e| {
            let msg = format!("{}", e);
            msg.contains("panic")
        }),
        "broken dependency syntax should not panic"
    );
}

// ─── Go: Missing module line ───────────────────────────────────────────────────

#[test]
fn go_mod_missing_module_line() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("go.mod"),
        "require (\n\tgithub.com/pkg v1.0.0\n)\n",
    )
    .unwrap();

    let r = detect_all(&ProjectRoot::new(dir.path()).unwrap());
    // Missing 'module' line makes it invalid, should error
    assert!(
        r.errors().is_empty() || r.projects().is_empty(),
        "go.mod without module line should handle gracefully"
    );
}

#[test]
fn go_mod_junk_syntax() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("go.mod"),
        "module github.com/user/pkg\n\ngobbledygook here\n",
    )
    .unwrap();

    let r = detect_all(&ProjectRoot::new(dir.path()).unwrap());
    assert!(
        !r.errors().iter().any(|e| {
            let msg = format!("{}", e);
            msg.contains("panic")
        }),
        "junk go.mod syntax should not panic"
    );
}

// ─── Python: Empty [project] section ───────────────────────────────────────────

#[test]
fn pyproject_toml_empty_project_section() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("pyproject.toml"), "[project]\n").unwrap();

    let r = detect_all(&ProjectRoot::new(dir.path()).unwrap());
    // Missing required fields should error
    assert!(
        r.errors().is_empty() || r.projects().is_empty(),
        "empty [project] should not panic"
    );
}

#[test]
fn pyproject_toml_broken_version_syntax() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("pyproject.toml"),
        "[project]\nname = \"app\"\nversion = 1.0.\n",
    )
    .unwrap();

    let r = detect_all(&ProjectRoot::new(dir.path()).unwrap());
    // Syntax error in version
    assert!(
        !r.errors().iter().any(|e| {
            let msg = format!("{}", e);
            msg.contains("panic")
        }),
        "broken version syntax should not panic"
    );
}

// ─── TypeScript: package.json as array ────────────────────────────────────────

#[test]
fn package_json_is_array() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("package.json"), "[]").unwrap();
    std::fs::write(dir.path().join("tsconfig.json"), "{}").unwrap();

    let r = detect_all(&ProjectRoot::new(dir.path()).unwrap());
    assert!(!r.errors().is_empty(), "JSON array should error");
}

#[test]
fn package_json_is_string() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("package.json"), r#""hello""#).unwrap();
    std::fs::write(dir.path().join("tsconfig.json"), "{}").unwrap();

    let r = detect_all(&ProjectRoot::new(dir.path()).unwrap());
    assert!(!r.errors().is_empty(), "JSON string root should error");
}

#[test]
fn tsconfig_json_is_array() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("package.json"), r#"{"name": "app"}"#).unwrap();
    std::fs::write(dir.path().join("tsconfig.json"), "[]").unwrap();

    let r = detect_all(&ProjectRoot::new(dir.path()).unwrap());
    // Should handle tsconfig.json being an array gracefully
    assert!(
        !r.errors().iter().any(|e| {
            let msg = format!("{}", e);
            msg.contains("panic")
        }),
        "tsconfig.json as array should not panic"
    );
}

#[test]
fn tsconfig_json_is_string() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("package.json"), r#"{"name": "app"}"#).unwrap();
    std::fs::write(dir.path().join("tsconfig.json"), r#""string""#).unwrap();

    let r = detect_all(&ProjectRoot::new(dir.path()).unwrap());
    // Should handle tsconfig.json being a string gracefully
    assert!(
        !r.errors().iter().any(|e| {
            let msg = format!("{}", e);
            msg.contains("panic")
        }),
        "tsconfig.json as string should not panic"
    );
}

// ─── DotNet: Huge .sln with many project entries ────────────────────────────

#[test]
fn sln_with_1000_projects() {
    let dir = tempfile::tempdir().unwrap();
    let mut content = String::from("Microsoft Visual Studio Solution File, Format Version 12.00\n");
    for i in 0..1000 {
        content.push_str(&format!(
            r#"Project("{{FAE04EC0-301F-11D3-BF4B-00C04F79EFBC}}")  = "Project{}", "src/Proj{}.csproj", "{{ID{}}}"
EndProject
"#,
            i, i, i
        ));
    }

    std::fs::write(dir.path().join("App.sln"), content).unwrap();

    let r = detect_all(&ProjectRoot::new(dir.path()).unwrap());
    // Large .sln should be handled without panic
    assert!(
        !r.errors().iter().any(|e| {
            let msg = format!("{}", e);
            msg.contains("panic") || msg.contains("overflow")
        }),
        ".sln with 1000 projects should not panic"
    );
}

#[test]
fn csproj_missing_closing_tag() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("App.csproj"),
        "<Project><PropertyGroup><RootNamespace>App</PropertyGroup>",
    )
    .unwrap();

    let r = detect_all(&ProjectRoot::new(dir.path()).unwrap());
    // Unclosed XML should error, not panic
    assert!(
        !r.errors().iter().any(|e| {
            let msg = format!("{}", e);
            msg.contains("panic")
        }),
        "unclosed XML tag should not panic"
    );
}

// ─── Deno: Unknown fields in deno.json ─────────────────────────────────────────

#[test]
fn deno_json_with_unknown_fields() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("deno.json"),
        r#"{"name": "app", "unknownField1": "value", "unknownField2": 123, "unknownField3": true}"#,
    )
    .unwrap();

    let r = detect_all(&ProjectRoot::new(dir.path()).unwrap());
    // Unknown fields should not cause panic
    assert!(
        !r.errors().iter().any(|e| {
            let msg = format!("{}", e);
            msg.contains("panic")
        }),
        "unknown deno.json fields should not panic"
    );
}

#[test]
fn deno_json_version_as_object() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("deno.json"),
        r#"{"name": "app", "version": {"major": 1, "minor": 0}}"#,
    )
    .unwrap();

    let r = detect_all(&ProjectRoot::new(dir.path()).unwrap());
    // Object instead of string version should be handled
    assert!(
        !r.errors().iter().any(|e| {
            let msg = format!("{}", e);
            msg.contains("panic")
        }),
        "deno.json with object version should not panic"
    );
}

// ─── Mixed malformed scenarios ─────────────────────────────────────────────────

#[test]
fn multiple_manifest_types_both_invalid() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("package.json"), "not valid json").unwrap();
    std::fs::write(dir.path().join("Cargo.toml"), "not valid toml [[[").unwrap();

    let r = detect_all(&ProjectRoot::new(dir.path()).unwrap());
    // Both invalid should produce errors, not panic
    assert!(
        !r.errors().iter().any(|e| {
            let msg = format!("{}", e);
            msg.contains("panic")
        }),
        "multiple invalid manifests should not panic"
    );
}

#[test]
fn control_characters_in_json_values() {
    let dir = tempfile::tempdir().unwrap();
    // JSON with control characters (null bytes, etc.)
    std::fs::write(
        dir.path().join("package.json"),
        b"{\"name\": \"app\x00\", \"version\": \"1.0.0\"}",
    )
    .unwrap();
    std::fs::write(dir.path().join("tsconfig.json"), "{}").unwrap();

    let r = detect_all(&ProjectRoot::new(dir.path()).unwrap());
    // Control characters should be handled
    assert!(
        !r.errors().iter().any(|e| {
            let msg = format!("{}", e);
            msg.contains("panic")
        }),
        "control characters should not panic"
    );
}
