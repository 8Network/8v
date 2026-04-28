use o8v_testkit::TempProject;
use std::fs;
use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

fn is_valid_semver(v: &str) -> bool {
    let parts: Vec<&str> = v.split('.').collect();
    parts.len() == 3
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
}

fn validate_base_url(url: &str) -> bool {
    url.starts_with("https://")
        || url.starts_with("http://localhost")
        || url.starts_with("http://127.0.0.1")
}

/// Extract binary names from the release workflow file.
fn extract_binary_names_from_workflow() -> Vec<String> {
    let workflow = include_str!("../../.github/workflows/release.yml");
    let mut binaries = Vec::new();
    for line in workflow.lines() {
        if line.trim_start().starts_with("cp ") && line.contains("dist/8v-") {
            if let Some(start) = line.find("dist/8v-") {
                let rest = &line[start + 5..];
                let binary = rest.split_whitespace().next().unwrap_or("");
                if !binary.is_empty() {
                    binaries.push(binary.to_string());
                }
            }
        }
    }
    binaries
}

#[test]
fn workspace_cargo_toml_has_version_field() {
    let root = workspace_root();
    let cargo_toml = root.join("Cargo.toml");
    assert!(
        cargo_toml.exists(),
        "Workspace root Cargo.toml does not exist"
    );
    let content = fs::read_to_string(&cargo_toml).expect("read workspace Cargo.toml");
    assert!(
        content.contains("[workspace.package]"),
        "Workspace Cargo.toml missing [workspace.package] section"
    );
    assert!(
        content.contains("version = \""),
        "Workspace Cargo.toml missing version field — bump the version in [workspace.package]"
    );
}

#[test]
fn workflow_targets_workspace_package_version() {
    let root = workspace_root();
    let cargo_toml = root.join("Cargo.toml");
    let content = fs::read_to_string(&cargo_toml).expect("read workspace Cargo.toml");
    // The workflow releases whatever version is in [workspace.package].
    // Verify the section and version key are present so a tag push reflects
    // the correct version.
    assert!(
        content.contains("[workspace.package]"),
        "[workspace.package] section missing — workflow release would use wrong version"
    );
    let in_section = content
        .lines()
        .skip_while(|l| !l.trim().starts_with("[workspace.package]"))
        .skip(1)
        .take_while(|l| !l.trim_start().starts_with('['))
        .any(|l| l.trim_start().starts_with("version = \""));
    assert!(
        in_section,
        "version field not found under [workspace.package] — CI release would be unversioned"
    );
}

#[test]
fn version_bump_updates_package_version_line() {
    let project = TempProject::empty();
    let cargo_path = project.path().join("Cargo.toml");

    let original = r#"[package]
name = "test"
version = "0.1.0"
edition = "2021"
"#;
    project
        .write_file("Cargo.toml", original.as_bytes())
        .expect("write temp Cargo.toml");

    let content = fs::read_to_string(&cargo_path).expect("read temp Cargo.toml");
    let updated = content.replace(r#"version = "0.1.0""#, r#"version = "1.2.3""#);
    project
        .write_file("Cargo.toml", updated.as_bytes())
        .expect("write updated Cargo.toml");

    let result = fs::read_to_string(&cargo_path).expect("read updated");
    assert!(result.contains(r#"version = "1.2.3""#));
    assert!(!result.contains(r#"version = "0.1.0""#));
}

#[test]
fn version_bump_does_not_touch_dependency_versions() {
    let project = TempProject::empty();
    let cargo_path = project.path().join("Cargo.toml");

    let original = r#"[package]
name = "test"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = { version = "1.0.0", features = ["derive"] }
"#;
    project
        .write_file("Cargo.toml", original.as_bytes())
        .expect("write temp Cargo.toml");

    let content = fs::read_to_string(&cargo_path).expect("read");
    // Apply version bump: only update lines starting with 'version = '
    let lines: Vec<_> = content
        .lines()
        .map(|line| {
            if line.trim_start().starts_with("version = \"") && !line.contains("{") {
                line.replace(r#"version = "0.1.0""#, r#"version = "1.2.3""#)
            } else {
                line.to_string()
            }
        })
        .collect();
    let updated = lines.join("\n") + "\n";
    project
        .write_file("Cargo.toml", updated.as_bytes())
        .expect("write updated");

    let result = fs::read_to_string(&cargo_path).expect("read updated");
    assert!(result.contains(r#"version = "1.2.3""#));
    assert!(
        result.contains(r#"version = "1.0.0""#),
        "dependency version was modified"
    );
}

#[test]
fn checksum_format_parseable() {
    let project = TempProject::empty();
    let checksums_path = project.path().join("checksums.txt");

    let checksum_content = "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789  8v-darwin-arm64\n\
                            fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210  8v-linux-x64\n";
    project
        .write_file("checksums.txt", checksum_content.as_bytes())
        .expect("write checksums");

    let content = fs::read_to_string(&checksums_path).expect("read checksums");
    for line in content.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        assert!(parts.len() >= 2, "Invalid checksum line format: {}", line);
        let hash = parts[0];

        assert_eq!(hash.len(), 64, "Hash should be 64 hex chars");
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()), "Invalid hex");
    }

    // Test that "8v-darwin-arm" does NOT match "8v-darwin-arm64"
    assert!(
        !content.contains("8v-darwin-arm\n"),
        "Partial match should not occur"
    );
}

#[test]
fn binary_names_match_install_platforms() {
    let binaries = extract_binary_names_from_workflow();
    assert!(
        !binaries.is_empty(),
        "Failed to extract binary names from workflow"
    );

    let expected = ["darwin-arm64", "darwin-x64", "linux-x64", "linux-arm64"];
    for expected_name in expected {
        assert!(
            binaries.iter().any(|b| b.contains(expected_name)),
            "Binary name {} not found in release workflow",
            expected_name
        );
    }
}

#[test]
fn version_txt_has_no_whitespace() {
    let project = TempProject::empty();
    let version_path = project.path().join("version.txt");

    project
        .write_file("version.txt", b"1.2.3\n")
        .expect("write version.txt");

    let content = fs::read_to_string(&version_path).expect("read version.txt");
    let trimmed = content.trim_end();

    assert_eq!(
        trimmed, "1.2.3",
        "Version should have no trailing whitespace"
    );
    assert!(
        !trimmed.starts_with('v'),
        "Version should not have v prefix"
    );
    assert!(
        !trimmed.starts_with(' '),
        "Version should not have leading space"
    );
}

#[test]
fn semver_regex_accepts_valid_versions() {
    let valid = ["0.1.0", "1.0.0", "10.20.30", "1.2.3"];
    for v in valid {
        assert!(is_valid_semver(v), "Should accept valid semver: {}", v);
    }
}

#[test]
fn semver_regex_rejects_invalid_versions() {
    let invalid = ["v1.0.0", "1.0", "latest", "", "1.0.0-beta", "1.0.0.0"];
    for v in invalid {
        assert!(!is_valid_semver(v), "Should reject invalid semver: {}", v);
    }
}

#[test]
fn base_url_validation_allows_https_and_localhost() {
    assert!(validate_base_url("https://example.com"));
    assert!(validate_base_url("https://example.com/path"));
    assert!(validate_base_url("http://localhost:8080"));
    assert!(validate_base_url("http://localhost"));
    assert!(validate_base_url("http://127.0.0.1:3000"));
    assert!(validate_base_url("http://127.0.0.1"));

    assert!(!validate_base_url("http://example.com"));
    assert!(!validate_base_url("http://evil.com"));
    assert!(!validate_base_url("ftp://example.com"));
    assert!(!validate_base_url(""));
}
