//! Validation tests — error paths, wrong types, structural edge cases.
//! Organized by stack. Each test goes through `detect_all()`.
//! Unit-level tests live in src/*.rs — this file covers integration-level behavior.

use o8v_project::{detect_all, ProjectKind, ProjectRoot, Stack};

// ─── Rust ──────────────────────────────────────────────────────────────────

#[test]
fn rust_non_string_version_is_error() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("Cargo.toml"),
        "[package]\nname = \"app\"\nversion = 42\n",
    )
    .unwrap();

    let r = detect_all(&ProjectRoot::new(dir.path()).unwrap());
    assert!(
        !r.errors().is_empty(),
        "version = 42 should produce an error"
    );
}

// ─── TypeScript ────────────────────────────────────────────────────────────

#[test]
fn javascript_workspace_detected_not_typescript() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("package.json"),
        r#"{"name": "js-monorepo", "workspaces": ["packages/*"]}"#,
    )
    .unwrap();

    let r = detect_all(&ProjectRoot::new(dir.path()).unwrap());

    // Should be JavaScript, not TypeScript
    assert!(
        !r.projects().iter().any(|p| p.stack() == Stack::TypeScript),
        "JS workspace without tsconfig should NOT be TypeScript"
    );

    let js: Vec<_> = r
        .projects()
        .iter()
        .filter(|p| p.stack() == Stack::JavaScript)
        .collect();
    assert_eq!(js.len(), 1, "JS workspace should be detected as JavaScript");
    assert_eq!(js[0].name(), "js-monorepo");
}

#[test]
fn package_json_without_name_is_not_an_error() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("package.json"),
        r#"{"private": true, "version": "1.0.0"}"#,
    )
    .unwrap();
    std::fs::write(dir.path().join("tsconfig.json"), "{}").unwrap();

    let r = detect_all(&ProjectRoot::new(dir.path()).unwrap());
    assert!(
        r.is_ok(),
        "private package without name should not error: {:?}",
        r.errors()
    );
}

#[test]
fn ts_non_string_name_is_error() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("package.json"), r#"{"name": 42}"#).unwrap();
    std::fs::write(dir.path().join("tsconfig.json"), "{}").unwrap();

    let r = detect_all(&ProjectRoot::new(dir.path()).unwrap());
    assert!(!r.errors().is_empty(), "name: 42 should produce an error");
}

#[test]
fn ts_non_object_json_is_error() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("package.json"), "[]").unwrap();
    std::fs::write(dir.path().join("tsconfig.json"), "{}").unwrap();

    let r = detect_all(&ProjectRoot::new(dir.path()).unwrap());
    assert!(
        !r.errors().is_empty(),
        "JSON array root should produce an error"
    );
}

#[test]
fn ts_empty_workspaces_is_standalone() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("package.json"),
        r#"{"name": "mono", "workspaces": []}"#,
    )
    .unwrap();
    std::fs::write(dir.path().join("tsconfig.json"), "{}").unwrap();

    let r = detect_all(&ProjectRoot::new(dir.path()).unwrap());
    assert!(
        r.errors().is_empty(),
        "empty workspaces [] should not error"
    );
    assert_eq!(
        r.projects().len(),
        1,
        "should detect one project with empty workspaces as Standalone"
    );
    if let crate::ProjectKind::Standalone = r.projects()[0].kind() {
        // OK
    } else {
        panic!("expected Standalone, got {:?}", r.projects()[0].kind());
    }
}

#[cfg(unix)]
#[test]
fn ts_dangling_symlink_tsconfig_is_error() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("package.json"),
        r#"{"name": "app", "version": "1.0.0"}"#,
    )
    .unwrap();
    std::os::unix::fs::symlink("/nonexistent/target", dir.path().join("tsconfig.json")).unwrap();

    let r = detect_all(&ProjectRoot::new(dir.path()).unwrap());
    assert!(
        !r.errors().is_empty(),
        "dangling tsconfig symlink should produce an error"
    );
}

// ─── Python ────────────────────────────────────────────────────────────────

#[test]
fn python_non_string_name_is_error() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("pyproject.toml"),
        "[project]\nname = 42\nversion = \"1.0.0\"\n",
    )
    .unwrap();

    let r = detect_all(&ProjectRoot::new(dir.path()).unwrap());
    assert!(!r.errors().is_empty(), "name = 42 should produce an error");
}

#[test]
fn python_poetry_non_string_version_is_error() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("pyproject.toml"),
        "[tool.poetry]\nname = \"pkg\"\nversion = 42\n",
    )
    .unwrap();

    let r = detect_all(&ProjectRoot::new(dir.path()).unwrap());
    assert!(
        !r.errors().is_empty(),
        "version = 42 should produce an error"
    );
}

#[test]
fn python_uv_not_table_is_error() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("pyproject.toml"),
        "[project]\nname = \"pkg\"\nversion = \"1.0.0\"\n\n[tool]\nuv = 42\n",
    )
    .unwrap();

    let r = detect_all(&ProjectRoot::new(dir.path()).unwrap());
    assert!(!r.errors().is_empty(), "uv = 42 should produce an error");
}

#[test]
fn python_poetry_not_table_is_error() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("pyproject.toml"), "[tool]\npoetry = 42\n").unwrap();

    let r = detect_all(&ProjectRoot::new(dir.path()).unwrap());
    assert!(
        !r.errors().is_empty(),
        "poetry = 42 should produce an error"
    );
}

#[test]
fn python_empty_uv_members_is_standalone() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("pyproject.toml"),
        "[project]\nname = \"pkg\"\nversion = \"1.0.0\"\n\n[tool.uv.workspace]\nmembers = []\n",
    )
    .unwrap();

    let r = detect_all(&ProjectRoot::new(dir.path()).unwrap());
    assert!(
        r.errors().is_empty(),
        "empty uv workspace members should not error"
    );
    assert_eq!(
        r.projects().len(),
        1,
        "should detect one project with empty uv workspace as Standalone"
    );
    if let crate::ProjectKind::Standalone = r.projects()[0].kind() {
        // OK
    } else {
        panic!("expected Standalone, got {:?}", r.projects()[0].kind());
    }
}

// ─── .NET ──────────────────────────────────────────────────────────────────

#[test]
fn sln_solution_folder_excluded_by_guid() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("Test.sln"),
        r#"
Project("{FAE04EC0-301F-11D3-BF4B-00C04F79EFBC}") = "App", "src\App\App.csproj", "{A1}"
EndProject
Project("{2150E333-8FDC-42A3-9474-1A3956D46DE8}") = "MyFolder", "MyFolder", "{GUID2}"
EndProject
Project("{2150E333-8FDC-42A3-9474-1A3956D46DE8}") = "Solution Items", "Solution Items", "{GUID3}"
EndProject
"#,
    )
    .unwrap();

    let r = detect_all(&ProjectRoot::new(dir.path()).unwrap());
    assert!(r.is_ok(), "errors: {:?}", r.errors());
    assert_eq!(r.projects().len(), 1);
    if let ProjectKind::Compound { members } = r.projects()[0].kind() {
        assert_eq!(members, &["App"], "solution folders should be excluded");
    } else {
        panic!("expected workspace");
    }
}

// ─── JavaScript: empty workspaces ───────────────────────────────────────────

#[test]
fn js_empty_workspaces_is_standalone() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("package.json"),
        r#"{"name": "mono", "workspaces": []}"#,
    )
    .unwrap();
    // No tsconfig — this is JavaScript

    let r = detect_all(&ProjectRoot::new(dir.path()).unwrap());
    assert!(
        r.errors().is_empty(),
        "empty JS workspaces [] should not error"
    );
    assert_eq!(
        r.projects().len(),
        1,
        "should detect one project with empty workspaces as Standalone"
    );
    if let crate::ProjectKind::Standalone = r.projects()[0].kind() {
        // OK
    } else {
        panic!("expected Standalone, got {:?}", r.projects()[0].kind());
    }
}

// ─── Rust: version.workspace = false ───────────────────────────────────────

#[test]
fn rust_version_workspace_false_is_error() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("Cargo.toml"),
        "[package]\nname = \"app\"\nversion.workspace = false\n",
    )
    .unwrap();

    let r = detect_all(&ProjectRoot::new(dir.path()).unwrap());
    assert!(
        !r.errors().is_empty(),
        "version.workspace = false should produce an error"
    );
}

// ─── DotNet: ambiguity ─────────────────────────────────────────────────────

#[test]
fn dotnet_multiple_sln_is_error() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("A.sln"),
        "Microsoft Visual Studio Solution File\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("B.sln"),
        "Microsoft Visual Studio Solution File\n",
    )
    .unwrap();

    let r = detect_all(&ProjectRoot::new(dir.path()).unwrap());
    assert!(
        r.errors().iter().any(|e| format!("{e}").contains(".sln")),
        "multiple .sln should error"
    );
}

#[test]
fn dotnet_multiple_csproj_is_error() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("A.csproj"), "<Project Sdk=\"Microsoft.NET.Sdk\"><PropertyGroup><RootNamespace>A</RootNamespace></PropertyGroup></Project>").unwrap();
    std::fs::write(dir.path().join("B.csproj"), "<Project Sdk=\"Microsoft.NET.Sdk\"><PropertyGroup><RootNamespace>B</RootNamespace></PropertyGroup></Project>").unwrap();

    let r = detect_all(&ProjectRoot::new(dir.path()).unwrap());
    assert!(
        r.errors()
            .iter()
            .any(|e| format!("{e}").contains(".csproj")),
        "multiple .csproj should error"
    );
}

// ─── Python: poetry without name ───────────────────────────────────────────

#[test]
fn python_poetry_without_name_is_error() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("pyproject.toml"),
        "[tool.poetry]\nversion = \"1.0.0\"\n",
    )
    .unwrap();

    let r = detect_all(&ProjectRoot::new(dir.path()).unwrap());
    assert!(
        !r.errors().is_empty(),
        "poetry section without name should error"
    );
}

// ─── Python: config-only ───────────────────────────────────────────────────

#[test]
fn python_config_only_is_not_a_project() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("pyproject.toml"),
        "[tool.ruff]\nline-length = 88\n",
    )
    .unwrap();

    let r = detect_all(&ProjectRoot::new(dir.path()).unwrap());
    assert!(r.is_ok(), "config-only should not error: {:?}", r.errors());
    assert!(
        r.projects().is_empty(),
        "config-only should not produce a project"
    );
}
