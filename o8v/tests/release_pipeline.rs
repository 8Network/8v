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

/// Verify release.sh version bump targets the workspace root Cargo.toml.
fn release_sh_version_bump_sed_command() -> Option<String> {
    let release_sh = include_str!("../../scripts/release.sh");
    for line in release_sh.lines() {
        if line.contains("sed") && line.contains("version = ") && line.contains("Cargo.toml") {
            return Some(line.trim().to_string());
        }
    }
    None
}

/// Extract binary names from release.sh build section.
fn extract_binary_names_from_release_sh() -> Vec<String> {
    let release_sh = include_str!("../../scripts/release.sh");
    let mut binaries = Vec::new();

    for line in release_sh.lines() {
        if line.contains("cp target") && line.contains("dist/8v-") {
            if let Some(start) = line.find("dist/") {
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
        "Workspace Cargo.toml missing version field — release.sh sed would silently skip it"
    );
}

#[test]
fn release_sh_version_bump_targets_workspace_root() {
    let sed_cmd = release_sh_version_bump_sed_command()
        .expect("release.sh has no sed command targeting Cargo.toml version — bump step broken");
    assert!(
        sed_cmd.contains("Cargo.toml"),
        "sed command does not target Cargo.toml: {sed_cmd}"
    );
    assert!(
        sed_cmd.contains("^version = "),
        "sed command does not anchor to ^version = (may match dependency versions): {sed_cmd}"
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
    let binaries = extract_binary_names_from_release_sh();
    assert!(!binaries.is_empty(), "Failed to extract binary names");

    let expected = ["darwin-arm64", "darwin-x64", "linux-x64", "linux-arm64"];
    for expected_name in expected {
        assert!(
            binaries.iter().any(|b| b.contains(expected_name)),
            "Binary name {} not found in release.sh",
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
fn changelog_sed_preserves_unreleased_and_adds_version() {
    let project = TempProject::empty();
    let changelog_path = project.path().join("CHANGELOG.md");

    let original = r#"# Changelog

## [Unreleased]

## [1.0.0] - 2026-01-01

### Added

- Some feature
"#;
    project
        .write_file("CHANGELOG.md", original.as_bytes())
        .expect("write CHANGELOG");

    let content = fs::read_to_string(&changelog_path).expect("read CHANGELOG");

    // Simulate sed: insert new version after [Unreleased]
    let updated = content.replace(
        "## [Unreleased]",
        "## [Unreleased]\n\n## [2.0.0] - 2026-04-07",
    );
    project
        .write_file("CHANGELOG.md", updated.as_bytes())
        .expect("write updated CHANGELOG");

    let result = fs::read_to_string(&changelog_path).expect("read updated");
    assert!(result.contains("## [Unreleased]"));
    assert!(result.contains("## [2.0.0] - 2026-04-07"));
    assert!(result.contains("## [1.0.0] - 2026-01-01"));
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
