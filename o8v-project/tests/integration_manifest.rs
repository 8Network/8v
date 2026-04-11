//! Edge case tests found during code review.

use o8v_project::{detect_all, ProjectKind, ProjectRoot, Stack};

// ─── #17: TypeScript workspace styles ──────────────────────────────────────

#[test]
fn ts_workspace_array_style() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("package.json"),
        r#"{"name": "mono", "workspaces": ["packages/*", "apps/*"]}"#,
    )
    .unwrap();
    std::fs::write(dir.path().join("tsconfig.json"), "{}").unwrap();

    let root = ProjectRoot::new(dir.path()).unwrap();
    let r = detect_all(&root);

    assert!(r.errors().is_empty(), "errors: {:?}", r.errors());
    assert_eq!(r.projects().len(), 1);
    assert_eq!(r.projects()[0].stack(), Stack::TypeScript);
    if let ProjectKind::Compound { members } = r.projects()[0].kind() {
        assert_eq!(members, &["packages/*", "apps/*"]);
    } else {
        panic!("expected workspace");
    }
}

#[test]
fn ts_workspace_object_style_yarn() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("package.json"),
        r#"{"name": "mono", "workspaces": {"packages": ["packages/*"]}}"#,
    )
    .unwrap();
    std::fs::write(dir.path().join("tsconfig.json"), "{}").unwrap();

    let root = ProjectRoot::new(dir.path()).unwrap();
    let r = detect_all(&root);

    assert!(r.errors().is_empty(), "errors: {:?}", r.errors());
    assert_eq!(r.projects().len(), 1);
    if let ProjectKind::Compound { members } = r.projects()[0].kind() {
        assert_eq!(members, &["packages/*"]);
    } else {
        panic!("expected workspace");
    }
}

#[test]
fn ts_workspace_wrong_type_is_error() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("package.json"),
        r#"{"name": "bad", "workspaces": true}"#,
    )
    .unwrap();
    std::fs::write(dir.path().join("tsconfig.json"), "{}").unwrap();

    let root = ProjectRoot::new(dir.path()).unwrap();
    let r = detect_all(&root);

    assert!(
        !r.errors().is_empty(),
        "workspaces: true should produce an error"
    );
}

// ─── #18: DotNet name extraction priority ──────────────────────────────────

#[test]
fn dotnet_rootnamespace_preferred() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("MyFile.csproj"),
        "<Project><PropertyGroup><RootNamespace>MyApp.Core</RootNamespace><AssemblyName>MyAssembly</AssemblyName></PropertyGroup></Project>",
    ).unwrap();

    let root = ProjectRoot::new(dir.path()).unwrap();
    let r = detect_all(&root);

    assert!(r.errors().is_empty(), "errors: {:?}", r.errors());
    assert_eq!(r.projects().len(), 1);
    assert_eq!(
        r.projects()[0].name(),
        "MyApp.Core",
        "RootNamespace should be preferred over AssemblyName"
    );
}

#[test]
fn dotnet_assemblyname_when_no_rootnamespace() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("MyFile.csproj"),
        "<Project><PropertyGroup><AssemblyName>MyAssembly</AssemblyName></PropertyGroup></Project>",
    )
    .unwrap();

    let root = ProjectRoot::new(dir.path()).unwrap();
    let r = detect_all(&root);

    assert!(r.errors().is_empty(), "errors: {:?}", r.errors());
    assert_eq!(r.projects().len(), 1);
    assert_eq!(r.projects()[0].name(), "MyAssembly");
}

#[test]
fn dotnet_csproj_no_name_fields_falls_back_to_filename() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("MyFile.csproj"),
        "<Project><PropertyGroup></PropertyGroup></Project>",
    )
    .unwrap();

    let root = ProjectRoot::new(dir.path()).unwrap();
    let r = detect_all(&root);

    // dotnet new console produces csproj without RootNamespace/AssemblyName
    // Should fall back to filename, like MSBuild does
    assert!(r.errors().is_empty(), "errors: {:?}", r.errors());
    assert_eq!(r.projects().len(), 1);
    assert_eq!(r.projects()[0].name(), "MyFile");
}

// ─── #19: Empty .sln (all Solution Items filtered) ─────────────────────────

#[test]
fn dotnet_sln_all_solution_items() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("Empty.sln"),
        r#"
Microsoft Visual Studio Solution File, Format Version 12.00
Project("{2150E333-8FDC-42A3-9474-1A3956D46DE8}") = "Solution Items", "Solution Items", "{A1}"
EndProject
"#,
    )
    .unwrap();

    let root = ProjectRoot::new(dir.path()).unwrap();
    let r = detect_all(&root);

    assert!(
        r.projects().is_empty(),
        "sln with only Solution Items should not produce a project"
    );
    assert!(
        r.errors().is_empty(),
        "sln with zero real projects should return None (no error) — solution folders are not projects"
    );
}

// ─── #20: Corrupt .csproj content ──────────────────────────────────────────

#[test]
fn dotnet_corrupt_csproj_is_error() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("Bad.csproj"), "this is not xml at all!!!").unwrap();

    let root = ProjectRoot::new(dir.path()).unwrap();
    let r = detect_all(&root);

    assert!(
        r.projects().is_empty(),
        "corrupt csproj should not produce a project"
    );
    assert!(
        !r.errors().is_empty(),
        "corrupt csproj should produce an error"
    );
}

// ─── #21: Relative path resolution in ProjectRoot ──────────────────────────

#[test]
fn project_path_resolves_relative() {
    // "." is relative — must become absolute
    let path = ProjectRoot::new(".").unwrap();
    assert!(
        path.as_containment_root().unwrap().as_path().is_absolute(),
        "relative '.' must resolve to absolute, got: {}",
        path
    );

    let cwd = std::env::current_dir().unwrap();
    assert_eq!(
        path.as_containment_root().unwrap().as_path(),
        cwd.as_path(),
        "'.' should resolve to current directory"
    );
}

#[test]
fn project_path_absolute_canonical() {
    let dir = tempfile::tempdir().unwrap();
    let canonical = std::fs::canonicalize(dir.path()).unwrap();
    let path = ProjectRoot::new(dir.path()).unwrap();
    assert_eq!(
        path.as_containment_root().unwrap().as_path(),
        canonical.as_path()
    );
}

// ─── DotNet: XML whitespace trimmed ────────────────────────────────────────

#[test]
fn dotnet_csproj_whitespace_in_name_trimmed() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("App.csproj"),
        "<Project Sdk=\"Microsoft.NET.Sdk\"><PropertyGroup><RootNamespace>  MyApp  </RootNamespace></PropertyGroup></Project>",
    ).unwrap();

    let r = detect_all(&ProjectRoot::new(dir.path()).unwrap());
    assert!(r.is_ok(), "errors: {:?}", r.errors());
    assert_eq!(
        r.projects()[0].name(),
        "MyApp",
        "XML whitespace should be trimmed"
    );
}

// ─── Python: uv workspace happy path ───────────────────────────────────────

#[test]
fn python_uv_workspace() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("pyproject.toml"),
        "[project]\nname = \"mono\"\nversion = \"1.0.0\"\n\n[tool.uv.workspace]\nmembers = [\"packages/a\", \"packages/b\"]\n",
    ).unwrap();

    let r = detect_all(&ProjectRoot::new(dir.path()).unwrap());
    assert!(r.is_ok(), "errors: {:?}", r.errors());
    assert_eq!(r.projects()[0].stack(), Stack::Python);
    if let ProjectKind::Compound { members } = r.projects()[0].kind() {
        assert_eq!(members, &["packages/a", "packages/b"]);
    } else {
        panic!("expected workspace");
    }
}

// ─── TypeScript: nohoist-only → Standalone ─────────────────────────────────

#[test]
fn ts_nohoist_only_is_standalone() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("package.json"),
        r#"{"name": "app", "workspaces": {"nohoist": ["**"]}}"#,
    )
    .unwrap();
    std::fs::write(dir.path().join("tsconfig.json"), "{}").unwrap();

    let r = detect_all(&ProjectRoot::new(dir.path()).unwrap());
    assert!(r.is_ok(), "errors: {:?}", r.errors());
    assert!(matches!(r.projects()[0].kind(), ProjectKind::Standalone));
}

// ─── Rust: virtual workspace name ──────────────────────────────────────────

#[test]
fn rust_virtual_workspace_has_meaningful_name() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("Cargo.toml"),
        "[workspace]\nmembers = [\"crates/a\"]\n",
    )
    .unwrap();

    let r = detect_all(&ProjectRoot::new(dir.path()).unwrap());
    assert!(r.is_ok(), "errors: {:?}", r.errors());
    let name = r.projects()[0].name();
    let expected = dir.path().file_name().unwrap().to_str().unwrap();
    assert_eq!(
        name, expected,
        "virtual workspace should use directory name"
    );
}

// ─── Directory named as manifest is an error, not silence ──────────────────

#[test]
fn directory_named_cargo_toml_is_error() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("Cargo.toml")).unwrap();

    let r = detect_all(&ProjectRoot::new(dir.path()).unwrap());
    assert!(
        !r.errors().is_empty(),
        "directory named Cargo.toml should produce an error, not silence"
    );
}

#[test]
fn directory_named_package_json_is_error() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("package.json")).unwrap();

    let r = detect_all(&ProjectRoot::new(dir.path()).unwrap());
    assert!(
        !r.errors().is_empty(),
        "directory named package.json should produce an error, not silence"
    );
}

// ─── Security: symlink containment ─────────────────────────────────────────

#[cfg(unix)]
#[test]
fn symlink_outside_project_is_error() {
    let dir = tempfile::tempdir().unwrap();
    // Symlink Cargo.toml to a file outside the project directory
    std::os::unix::fs::symlink("/etc/hosts", dir.path().join("Cargo.toml")).unwrap();

    let r = detect_all(&ProjectRoot::new(dir.path()).unwrap());
    assert!(
        !r.errors().is_empty(),
        "symlink outside project should produce an error"
    );
    // Error message should NOT contain the resolved target path
    let error_msg = format!("{}", r.errors()[0]);
    assert!(
        !error_msg.contains("/etc/hosts"),
        "error should not leak the symlink target path"
    );
}

// ─── Security: control characters in manifest names ────────────────────────

#[test]
fn manifest_with_ansi_escape_in_name_is_error() {
    let dir = tempfile::tempdir().unwrap();
    // package.json with ANSI escape in name
    std::fs::write(
        dir.path().join("package.json"),
        "{\"name\": \"bad\\u001b[31mname\", \"version\": \"1.0.0\"}",
    )
    .unwrap();

    let r = detect_all(&ProjectRoot::new(dir.path()).unwrap());
    // Should either error or have the name sanitized
    assert!(
        r.errors()
            .iter()
            .any(|e| format!("{e}").contains("control characters"))
            || r.projects().is_empty(),
        "ANSI escape in name should be caught"
    );
}
